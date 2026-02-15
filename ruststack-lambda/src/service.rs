//! Lambda service implementation

use crate::function::{Function, FunctionCode, FunctionConfig, FunctionState};
use crate::invocation::{InvocationError, InvocationResult, InvocationType};
use base64::{Engine, engine::general_purpose};
use bytes::Bytes;
use dashmap::DashMap;
use sha2::{Sha256, Digest};
use std::sync::Arc;
use thiserror::Error;
use tracing::{info, warn};

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
}

/// Lambda service managing functions and invocations
pub struct LambdaService {
    functions: DashMap<String, Arc<Function>>,
    // TODO: Add container manager for actual execution
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
        }
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

        // Calculate code hash and size
        let (code_sha256, code_size) = match &code {
            FunctionCode::ZipFile(data) => {
                let mut hasher = Sha256::new();
                hasher.update(data);
                let hash = general_purpose::STANDARD.encode(hasher.finalize());
                (hash, data.len() as i64)
            }
            FunctionCode::S3 { .. } => {
                // TODO: Fetch from S3 and calculate
                ("placeholder".to_string(), 0)
            }
        };

        let function = Arc::new(Function::new(config.clone(), code, code_sha256, code_size));

        info!(
            function_name = %config.function_name,
            runtime = %config.runtime.as_str(),
            "Created function"
        );

        self.functions.insert(config.function_name.clone(), function.clone());

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

        info!(function_name = %name, "Deleted function");
        Ok(())
    }

    /// List all functions
    pub fn list_functions(&self) -> Vec<Arc<Function>> {
        self.functions.iter().map(|r| r.value().clone()).collect()
    }

    /// Invoke a function
    pub async fn invoke(
        &self,
        function_name: &str,
        payload: Bytes,
        invocation_type: InvocationType,
    ) -> Result<InvocationResult, LambdaServiceError> {
        let function = self.functions.get(function_name)
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
                self.invoke_sync(&function, payload).await
            }
            InvocationType::RequestResponse => {
                self.invoke_sync(&function, payload).await
            }
        }
    }

    /// Synchronous invocation
    async fn invoke_sync(
        &self,
        function: &Function,
        payload: Bytes,
    ) -> Result<InvocationResult, LambdaServiceError> {
        // TODO: Implement actual container-based execution
        // For now, return a mock response indicating not implemented

        warn!(
            function_name = %function.config.function_name,
            "Container execution not yet implemented"
        );

        // Return an error response that indicates the feature isn't ready
        Ok(InvocationResult {
            status_code: 200,
            payload: Some(Bytes::from(
                r#"{"errorMessage":"Container execution not yet implemented","errorType":"NotImplemented"}"#
            )),
            function_error: Some("Unhandled".to_string()),
            log_result: None,
            executed_version: function.version.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function::Runtime;

    #[tokio::test]
    async fn test_create_and_get_function() {
        let service = LambdaService::new();

        let config = FunctionConfig {
            function_name: "test-function".to_string(),
            runtime: Runtime::Python312,
            handler: "app.handler".to_string(),
            ..Default::default()
        };

        let code = FunctionCode::ZipFile(b"fake zip content".to_vec());

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

        service.create_function(config, FunctionCode::ZipFile(vec![])).await.unwrap();

        service.delete_function("to-delete").unwrap();
        assert!(service.get_function("to-delete").is_none());
    }

    #[tokio::test]
    async fn test_function_not_found() {
        let service = LambdaService::new();

        let result = service.delete_function("nonexistent");
        assert!(matches!(result, Err(LambdaServiceError::FunctionNotFound(_))));
    }
}
