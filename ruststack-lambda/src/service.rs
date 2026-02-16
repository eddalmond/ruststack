//! Lambda service implementation

use crate::function::{Function, FunctionCode, FunctionConfig, FunctionState, Runtime};
use crate::invocation::{InvocationError, InvocationResult, InvocationType, LambdaContext};
use base64::{engine::general_purpose, Engine};
use bytes::Bytes;
use chrono::Utc;
use dashmap::DashMap;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use thiserror::Error;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

#[derive(Debug, Error)]
pub enum LambdaServiceError {
    #[error("Function already exists: {0}")]
    FunctionExists(String),

    #[error("Function not found: {0}")]
    FunctionNotFound(String),

    #[error("Invalid runtime: {0}")]
    InvalidRuntime(String),

    #[error("Invalid handler format")]
    InvalidHandler,

    #[error("Code size exceeds limit")]
    CodeTooLarge,

    #[error("Invocation error: {0}")]
    Invocation(#[from] InvocationError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Zip extraction error: {0}")]
    ZipError(String),
}

/// Lambda service managing functions and invocations
pub struct LambdaService {
    functions: DashMap<String, Arc<Function>>,
    /// Directory where extracted function code is stored
    code_dir: TempDir,
}

impl Default for LambdaService {
    fn default() -> Self {
        Self::new()
    }
}

impl LambdaService {
    pub fn new() -> Self {
        Self {
            functions: DashMap::new(),
            code_dir: TempDir::new().expect("Failed to create temp dir for Lambda code"),
        }
    }

    /// Get the code directory path for a function
    fn function_code_path(&self, function_name: &str) -> PathBuf {
        self.code_dir.path().join(function_name)
    }

    /// Extract zip code to the function's code directory
    fn extract_code(
        &self,
        function_name: &str,
        zip_data: &[u8],
    ) -> Result<PathBuf, LambdaServiceError> {
        let code_path = self.function_code_path(function_name);

        // Remove old code if exists
        if code_path.exists() {
            std::fs::remove_dir_all(&code_path)?;
        }
        std::fs::create_dir_all(&code_path)?;

        // Extract zip
        let cursor = std::io::Cursor::new(zip_data);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| LambdaServiceError::ZipError(e.to_string()))?;

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| LambdaServiceError::ZipError(e.to_string()))?;

            let outpath = match file.enclosed_name() {
                Some(path) => code_path.join(path),
                None => continue,
            };

            if file.is_dir() {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        std::fs::create_dir_all(p)?;
                    }
                }
                let mut outfile = std::fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }

            // Set permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode))?;
                }
            }
        }

        debug!(path = ?code_path, "Extracted function code");
        Ok(code_path)
    }

    /// Create a new function
    pub async fn create_function(
        &self,
        config: FunctionConfig,
        code: FunctionCode,
    ) -> Result<Arc<Function>, LambdaServiceError> {
        // Check if function already exists
        if self.functions.contains_key(&config.function_name) {
            return Err(LambdaServiceError::FunctionExists(config.function_name));
        }

        // Calculate code hash and size, extract if needed
        let (code_sha256, code_size, _code_path) = match &code {
            FunctionCode::ZipFile(data) => {
                let mut hasher = Sha256::new();
                hasher.update(data);
                let hash = general_purpose::STANDARD.encode(hasher.finalize());
                let size = data.len() as i64;

                // Extract the code
                let path = self.extract_code(&config.function_name, data)?;

                (hash, size, Some(path))
            }
            FunctionCode::S3 { .. } => {
                // TODO: Fetch from S3 and calculate
                ("placeholder".to_string(), 0, None)
            }
        };

        let function = Arc::new(Function::new(config.clone(), code, code_sha256, code_size));

        info!(
            function_name = %config.function_name,
            runtime = %config.runtime.as_str(),
            "Created function"
        );

        self.functions
            .insert(config.function_name.clone(), function.clone());

        Ok(function)
    }

    /// Get a function by name
    pub fn get_function(&self, name: &str) -> Option<Arc<Function>> {
        self.functions.get(name).map(|f| f.clone())
    }

    /// Delete a function
    pub fn delete_function(&self, name: &str) -> Result<(), LambdaServiceError> {
        if self.functions.remove(name).is_none() {
            return Err(LambdaServiceError::FunctionNotFound(name.to_string()));
        }

        // Clean up code directory
        let code_path = self.function_code_path(name);
        if code_path.exists() {
            let _ = std::fs::remove_dir_all(&code_path);
        }

        info!(function_name = %name, "Deleted function");
        Ok(())
    }

    /// List all functions
    pub fn list_functions(&self) -> Vec<Arc<Function>> {
        self.functions.iter().map(|r| r.value().clone()).collect()
    }

    /// Update function code
    pub async fn update_function_code(
        &self,
        function_name: &str,
        code: FunctionCode,
    ) -> Result<Arc<Function>, LambdaServiceError> {
        let existing = self
            .functions
            .get(function_name)
            .ok_or_else(|| LambdaServiceError::FunctionNotFound(function_name.to_string()))?;

        // Calculate new code hash and size
        let (code_sha256, code_size) = match &code {
            FunctionCode::ZipFile(data) => {
                let mut hasher = Sha256::new();
                hasher.update(data);
                let hash = general_purpose::STANDARD.encode(hasher.finalize());
                let size = data.len() as i64;

                // Extract the code
                self.extract_code(function_name, data)?;

                (hash, size)
            }
            FunctionCode::S3 { .. } => {
                // TODO: Fetch from S3
                ("placeholder".to_string(), 0)
            }
        };

        // Create updated function
        let updated = Arc::new(Function {
            config: existing.config.clone(),
            code,
            code_sha256,
            code_size,
            state: FunctionState::Active,
            last_modified: Utc::now(),
            version: existing.version.clone(),
            arn: existing.arn.clone(),
        });

        self.functions
            .insert(function_name.to_string(), updated.clone());

        info!(function_name = %function_name, "Updated function code");
        Ok(updated)
    }

    /// Update function configuration
    #[allow(clippy::too_many_arguments)]
    pub async fn update_function_configuration(
        &self,
        function_name: &str,
        description: Option<String>,
        handler: Option<String>,
        memory_size: Option<i32>,
        role: Option<String>,
        runtime: Option<Runtime>,
        timeout: Option<i32>,
        environment: Option<HashMap<String, String>>,
    ) -> Result<Arc<Function>, LambdaServiceError> {
        let existing = self
            .functions
            .get(function_name)
            .ok_or_else(|| LambdaServiceError::FunctionNotFound(function_name.to_string()))?;

        let mut config = existing.config.clone();
        if let Some(d) = description {
            config.description = Some(d);
        }
        if let Some(h) = handler {
            config.handler = h;
        }
        if let Some(m) = memory_size {
            config.memory_size = m;
        }
        if let Some(r) = role {
            config.role = r;
        }
        if let Some(rt) = runtime {
            config.runtime = rt;
        }
        if let Some(t) = timeout {
            config.timeout = t;
        }
        if let Some(env) = environment {
            config.environment = env;
        }

        let updated = Arc::new(Function {
            config,
            code: existing.code.clone(),
            code_sha256: existing.code_sha256.clone(),
            code_size: existing.code_size,
            state: FunctionState::Active,
            last_modified: Utc::now(),
            version: existing.version.clone(),
            arn: existing.arn.clone(),
        });

        self.functions
            .insert(function_name.to_string(), updated.clone());

        info!(function_name = %function_name, "Updated function configuration");
        Ok(updated)
    }

    /// Invoke a function
    pub async fn invoke(
        &self,
        function_name: &str,
        payload: Bytes,
        invocation_type: InvocationType,
    ) -> Result<InvocationResult, LambdaServiceError> {
        let function = self
            .functions
            .get(function_name)
            .ok_or_else(|| LambdaServiceError::FunctionNotFound(function_name.to_string()))?;

        // Check function state
        if function.state != FunctionState::Active {
            return Err(InvocationError::FunctionNotActive(function.state).into());
        }

        match invocation_type {
            InvocationType::DryRun => {
                // Just validate, don't actually invoke
                Ok(InvocationResult {
                    status_code: 204,
                    payload: None,
                    function_error: None,
                    log_result: None,
                    executed_version: function.version.clone(),
                })
            }
            InvocationType::Event => {
                // Async invocation - queue and return immediately
                warn!("Async invocation not yet implemented, treating as sync");
                self.invoke_subprocess(&function, payload).await
            }
            InvocationType::RequestResponse => self.invoke_subprocess(&function, payload).await,
        }
    }

    /// Invoke function using subprocess (Python support)
    async fn invoke_subprocess(
        &self,
        function: &Function,
        payload: Bytes,
    ) -> Result<InvocationResult, LambdaServiceError> {
        let code_path = self.function_code_path(&function.config.function_name);

        if !code_path.exists() {
            return Err(LambdaServiceError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Function code not found",
            )));
        }

        // Parse handler: module.function_name
        let handler_parts: Vec<&str> = function.config.handler.rsplitn(2, '.').collect();
        if handler_parts.len() != 2 {
            return Err(LambdaServiceError::InvalidHandler);
        }
        let handler_function = handler_parts[0];
        let handler_module = handler_parts[1].replace('.', "/");

        // Generate request ID
        let request_id = uuid::Uuid::new_v4().to_string();
        let deadline_ms = Utc::now().timestamp_millis() + (function.config.timeout as i64 * 1000);

        // Build Lambda context
        let context = LambdaContext::new(function, &request_id, deadline_ms);

        // Create a wrapper script that invokes the handler
        let wrapper_script = format!(
            r#"
import sys
import json
import os

# Add code path to Python path
sys.path.insert(0, "{code_path}")

# Set up Lambda context
class LambdaContext:
    def __init__(self):
        self.function_name = "{function_name}"
        self.function_version = "{version}"
        self.invoked_function_arn = "{arn}"
        self.memory_limit_in_mb = {memory}
        self.aws_request_id = "{request_id}"
        self.log_group_name = "/aws/lambda/{function_name}"
        self.log_stream_name = "2024/01/01/[$LATEST]{request_id_short}"
        self.identity = None
        self.client_context = None

    def get_remaining_time_in_millis(self):
        return {remaining_time}

# Import and call handler
try:
    from {module} import {handler}
    
    event = json.loads('{event_json}')
    context = LambdaContext()
    
    result = {handler}(event, context)
    
    # Ensure result is JSON serializable
    if result is not None:
        print(json.dumps(result))
    else:
        print("null")
except Exception as e:
    import traceback
    error_response = {{
        "errorMessage": str(e),
        "errorType": type(e).__name__,
        "stackTrace": traceback.format_exc().split("\\n")
    }}
    print(json.dumps(error_response))
    sys.exit(1)
"#,
            code_path = code_path.display(),
            function_name = function.config.function_name,
            version = function.version,
            arn = function.arn,
            memory = function.config.memory_size,
            request_id = request_id,
            request_id_short = &request_id[..8],
            remaining_time = context.get_remaining_time_in_millis(),
            module = handler_module,
            handler = handler_function,
            event_json = String::from_utf8_lossy(&payload)
                .replace('\'', "\\'")
                .replace('\n', "\\n"),
        );

        // Write wrapper script to temp file
        let script_path = code_path.join("__ruststack_invoke__.py");
        {
            let mut file = std::fs::File::create(&script_path)?;
            file.write_all(wrapper_script.as_bytes())?;
        }

        // Determine Python executable based on runtime
        let python_exe = match function.config.runtime {
            Runtime::Python39 => "python3.9",
            Runtime::Python310 => "python3.10",
            Runtime::Python311 => "python3.11",
            Runtime::Python312 => "python3.12",
            Runtime::Python313 => "python3.13",
            _ => "python3",
        };

        // Build environment variables
        let mut env_vars: HashMap<String, String> = function.config.environment.clone();
        env_vars.insert(
            "AWS_LAMBDA_FUNCTION_NAME".to_string(),
            function.config.function_name.clone(),
        );
        env_vars.insert(
            "AWS_LAMBDA_FUNCTION_VERSION".to_string(),
            function.version.clone(),
        );
        env_vars.insert(
            "AWS_LAMBDA_FUNCTION_MEMORY_SIZE".to_string(),
            function.config.memory_size.to_string(),
        );
        env_vars.insert(
            "AWS_LAMBDA_LOG_GROUP_NAME".to_string(),
            format!("/aws/lambda/{}", function.config.function_name),
        );
        env_vars.insert(
            "AWS_LAMBDA_LOG_STREAM_NAME".to_string(),
            format!("2024/01/01/[$LATEST]{}", &request_id[..8]),
        );
        env_vars.insert("AWS_REGION".to_string(), "us-east-1".to_string());
        env_vars.insert("AWS_DEFAULT_REGION".to_string(), "us-east-1".to_string());
        env_vars.insert("_HANDLER".to_string(), function.config.handler.clone());
        env_vars.insert(
            "LAMBDA_TASK_ROOT".to_string(),
            code_path.to_string_lossy().to_string(),
        );
        // Set LocalStack-compatible endpoint for S3/DynamoDB access
        env_vars.insert(
            "AWS_ENDPOINT_URL".to_string(),
            "http://localhost:4566".to_string(),
        );
        env_vars.insert("LOCALSTACK_HOSTNAME".to_string(), "localhost".to_string());

        debug!(
            function = %function.config.function_name,
            handler = %function.config.handler,
            python = %python_exe,
            "Invoking function via subprocess"
        );

        // Execute Python subprocess
        let output = Command::new(python_exe)
            .arg(&script_path)
            .current_dir(&code_path)
            .envs(&env_vars)
            .output()
            .await;

        // Clean up wrapper script
        let _ = std::fs::remove_file(&script_path);

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                if !stderr.is_empty() {
                    debug!(stderr = %stderr, "Function stderr");
                }

                if output.status.success() {
                    // Parse response - take the last non-empty line as JSON response
                    let response_line = stdout.lines().last().unwrap_or("null");
                    Ok(InvocationResult {
                        status_code: 200,
                        payload: Some(Bytes::from(response_line.to_string())),
                        function_error: None,
                        log_result: if !stderr.is_empty() {
                            Some(general_purpose::STANDARD.encode(stderr.as_bytes()))
                        } else {
                            None
                        },
                        executed_version: function.version.clone(),
                    })
                } else {
                    // Function returned an error
                    let error_output = if !stdout.is_empty() {
                        stdout.to_string()
                    } else {
                        format!(
                            r#"{{"errorMessage":"{}","errorType":"Runtime.ExitError"}}"#,
                            stderr.lines().last().unwrap_or("Unknown error")
                        )
                    };

                    Ok(InvocationResult {
                        status_code: 200,
                        payload: Some(Bytes::from(error_output)),
                        function_error: Some("Unhandled".to_string()),
                        log_result: Some(
                            general_purpose::STANDARD.encode(format!("{}\n{}", stdout, stderr)),
                        ),
                        executed_version: function.version.clone(),
                    })
                }
            }
            Err(e) => {
                error!(error = %e, "Failed to execute Python subprocess");

                // Check if it's a "command not found" error
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(InvocationResult {
                        status_code: 200,
                        payload: Some(Bytes::from(format!(
                            r#"{{"errorMessage":"Runtime {} not found. Ensure Python is installed.","errorType":"Runtime.InvalidRuntime"}}"#,
                            python_exe
                        ))),
                        function_error: Some("Unhandled".to_string()),
                        log_result: None,
                        executed_version: function.version.clone(),
                    })
                } else {
                    Err(LambdaServiceError::Io(e))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_get_function() {
        let service = LambdaService::new();

        let config = FunctionConfig {
            function_name: "test-function".to_string(),
            runtime: Runtime::Python312,
            handler: "app.handler".to_string(),
            ..Default::default()
        };

        let code = FunctionCode::ZipFile(create_test_zip());

        let function = service.create_function(config, code).await.unwrap();
        assert_eq!(function.config.function_name, "test-function");

        let retrieved = service.get_function("test-function").unwrap();
        assert_eq!(retrieved.config.function_name, "test-function");
    }

    #[tokio::test]
    async fn test_delete_function() {
        let service = LambdaService::new();

        let config = FunctionConfig {
            function_name: "to-delete".to_string(),
            runtime: Runtime::Python312,
            handler: "app.handler".to_string(),
            ..Default::default()
        };

        service
            .create_function(config, FunctionCode::ZipFile(create_test_zip()))
            .await
            .unwrap();

        service.delete_function("to-delete").unwrap();
        assert!(service.get_function("to-delete").is_none());
    }

    #[tokio::test]
    async fn test_function_not_found() {
        let service = LambdaService::new();

        let result = service.delete_function("nonexistent");
        assert!(matches!(
            result,
            Err(LambdaServiceError::FunctionNotFound(_))
        ));
    }

    #[tokio::test]
    async fn test_list_functions() {
        let service = LambdaService::new();

        // Create two functions
        for name in ["func1", "func2"] {
            let config = FunctionConfig {
                function_name: name.to_string(),
                runtime: Runtime::Python312,
                handler: "app.handler".to_string(),
                ..Default::default()
            };
            service
                .create_function(config, FunctionCode::ZipFile(create_test_zip()))
                .await
                .unwrap();
        }

        let functions = service.list_functions();
        assert_eq!(functions.len(), 2);
    }

    /// Create a minimal valid zip file for testing
    fn create_test_zip() -> Vec<u8> {
        let mut buffer = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("app.py", options).unwrap();
            zip.write_all(b"def handler(event, context):\n    return {'statusCode': 200}\n")
                .unwrap();
            zip.finish().unwrap();
        }
        buffer
    }
}
