//! Docker-based Lambda execution
//!
//! Provides isolated Lambda execution using Docker containers with AWS Lambda base images.
//! Supports warm container pooling to reduce cold start latency.

use crate::function::{Function, Runtime};
use crate::invocation::InvocationResult;
use base64::{engine::general_purpose, Engine};
use bytes::Bytes;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Docker execution errors
#[derive(Debug, Error)]
pub enum DockerError {
    #[error("Docker not available: {0}")]
    NotAvailable(String),

    #[error("Failed to start container: {0}")]
    StartFailed(String),

    #[error("Container execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Container timeout")]
    Timeout,

    #[error("Image pull failed: {0}")]
    ImagePullFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Lambda execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutorMode {
    /// Use subprocess execution (fast, no isolation)
    #[default]
    Subprocess,
    /// Use Docker containers (isolated, slower cold start)
    Docker,
    /// Automatically choose based on function config
    Auto,
}

impl ExecutorMode {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "subprocess" | "process" | "native" => Some(Self::Subprocess),
            "docker" | "container" => Some(Self::Docker),
            "auto" | "hybrid" => Some(Self::Auto),
            _ => None,
        }
    }
}

/// Get the Lambda base image for a runtime
pub fn runtime_image(runtime: &Runtime) -> &'static str {
    match runtime {
        Runtime::Python39 => "public.ecr.aws/lambda/python:3.9",
        Runtime::Python310 => "public.ecr.aws/lambda/python:3.10",
        Runtime::Python311 => "public.ecr.aws/lambda/python:3.11",
        Runtime::Python312 => "public.ecr.aws/lambda/python:3.12",
        Runtime::Python313 => "public.ecr.aws/lambda/python:3.13",
        Runtime::Nodejs18 => "public.ecr.aws/lambda/nodejs:18",
        Runtime::Nodejs20 => "public.ecr.aws/lambda/nodejs:20",
        Runtime::ProvidedAl2 => "public.ecr.aws/lambda/provided:al2",
        Runtime::ProvidedAl2023 => "public.ecr.aws/lambda/provided:al2023",
    }
}

/// Warm container entry
struct WarmContainer {
    container_id: String,
    function_name: String,
    runtime: Runtime,
    code_hash: String,
    last_used: Instant,
}

/// Docker executor configuration
#[derive(Debug, Clone)]
pub struct DockerExecutorConfig {
    /// How long to keep containers warm
    pub container_ttl: Duration,
    /// Maximum concurrent containers
    pub max_containers: usize,
    /// Network mode (host, bridge)
    pub network_mode: String,
    /// RustStack endpoint for containers to reach
    pub ruststack_endpoint: String,
}

impl Default for DockerExecutorConfig {
    fn default() -> Self {
        Self {
            container_ttl: Duration::from_secs(300), // 5 minutes
            max_containers: 10,
            network_mode: "bridge".to_string(),
            // For Docker Desktop (Mac/Windows), containers use host.docker.internal
            // For Linux with host network, use localhost
            ruststack_endpoint: "http://host.docker.internal:4566".to_string(),
        }
    }
}

/// Docker-based Lambda executor
pub struct DockerExecutor {
    config: DockerExecutorConfig,
    warm_containers: Arc<RwLock<Vec<WarmContainer>>>,
    docker_available: bool,
}

impl DockerExecutor {
    /// Create a new Docker executor
    pub async fn new(config: DockerExecutorConfig) -> Self {
        let docker_available = Self::check_docker().await;
        if !docker_available {
            warn!("Docker not available - Docker execution mode will fail");
        }

        let executor = Self {
            config,
            warm_containers: Arc::new(RwLock::new(Vec::new())),
            docker_available,
        };

        // Start cleanup task
        executor.start_cleanup_task();

        executor
    }

    /// Check if Docker is available
    async fn check_docker() -> bool {
        match Command::new("docker").arg("info").output().await {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    /// Returns whether Docker is available
    pub fn is_available(&self) -> bool {
        self.docker_available
    }

    /// Start background task to cleanup expired containers
    fn start_cleanup_task(&self) {
        let containers = self.warm_containers.clone();
        let ttl = self.config.container_ttl;

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;

                let mut to_remove = Vec::new();

                {
                    let containers = containers.read().await;
                    let now = Instant::now();

                    for container in containers.iter() {
                        if now.duration_since(container.last_used) > ttl {
                            to_remove.push(container.container_id.clone());
                        }
                    }
                }

                // Remove expired containers
                for container_id in to_remove {
                    debug!(container_id = %container_id, "Removing expired warm container");
                    let _ = Command::new("docker")
                        .args(["rm", "-f", &container_id])
                        .output()
                        .await;

                    let mut containers = containers.write().await;
                    containers.retain(|c| c.container_id != container_id);
                }
            }
        });
    }

    /// Find or create a warm container for the function
    async fn get_container(
        &self,
        function: &Function,
        code_path: &Path,
    ) -> Result<String, DockerError> {
        // Check for existing warm container
        {
            let mut containers = self.warm_containers.write().await;
            if let Some(idx) = containers.iter().position(|c| {
                c.function_name == function.config.function_name
                    && c.code_hash == function.code_sha256
            }) {
                let container = &mut containers[idx];
                container.last_used = Instant::now();
                debug!(
                    container_id = %container.container_id,
                    function = %function.config.function_name,
                    "Reusing warm container"
                );
                return Ok(container.container_id.clone());
            }
        }

        // Need to create a new container
        self.create_container(function, code_path).await
    }

    /// Create a new Docker container for the function
    async fn create_container(
        &self,
        function: &Function,
        code_path: &Path,
    ) -> Result<String, DockerError> {
        let image = runtime_image(&function.config.runtime);

        // Ensure image is available
        self.ensure_image(image).await?;

        // Build container name
        let container_name = format!(
            "ruststack-lambda-{}-{}",
            function.config.function_name,
            &uuid::Uuid::new_v4().to_string()[..8]
        );

        // Build environment variables
        let mut env_args = Vec::new();
        for (key, value) in &function.config.environment {
            env_args.push("-e".to_string());
            env_args.push(format!("{}={}", key, value));
        }

        // Add Lambda-specific env vars
        let lambda_env = [
            ("AWS_LAMBDA_FUNCTION_NAME", function.config.function_name.as_str()),
            ("AWS_LAMBDA_FUNCTION_VERSION", function.version.as_str()),
            (
                "AWS_LAMBDA_FUNCTION_MEMORY_SIZE",
                &function.config.memory_size.to_string(),
            ),
            ("AWS_REGION", "us-east-1"),
            ("AWS_DEFAULT_REGION", "us-east-1"),
            ("_HANDLER", &function.config.handler),
            ("AWS_ENDPOINT_URL", &self.config.ruststack_endpoint),
            ("LOCALSTACK_HOSTNAME", "host.docker.internal"),
        ];

        for (key, value) in lambda_env {
            env_args.push("-e".to_string());
            env_args.push(format!("{}={}", key, value));
        }

        // Build docker run command
        let mut args = vec![
            "create".to_string(),
            "--name".to_string(),
            container_name.clone(),
            "--network".to_string(),
            self.config.network_mode.clone(),
            "-v".to_string(),
            format!("{}:/var/task:ro", code_path.display()),
            "--memory".to_string(),
            format!("{}m", function.config.memory_size),
            "--cpus".to_string(),
            "1".to_string(),
        ];

        args.extend(env_args);
        args.push(image.to_string());
        args.push(function.config.handler.clone());

        debug!(args = ?args, "Creating Docker container");

        let output = Command::new("docker")
            .args(&args)
            .output()
            .await
            .map_err(|e| DockerError::StartFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!(stderr = %stderr, "Failed to create container");
            return Err(DockerError::StartFailed(stderr.to_string()));
        }

        let container_id = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_string();

        info!(
            container_id = %container_id,
            function = %function.config.function_name,
            image = %image,
            "Created new container"
        );

        // Add to warm pool
        {
            let mut containers = self.warm_containers.write().await;

            // Check if we're at capacity
            if containers.len() >= self.config.max_containers {
                // Remove oldest container
                if let Some(oldest) = containers
                    .iter()
                    .min_by_key(|c| c.last_used)
                    .map(|c| c.container_id.clone())
                {
                    let _ = Command::new("docker")
                        .args(["rm", "-f", &oldest])
                        .output()
                        .await;
                    containers.retain(|c| c.container_id != oldest);
                }
            }

            containers.push(WarmContainer {
                container_id: container_id.clone(),
                function_name: function.config.function_name.clone(),
                runtime: function.config.runtime,
                code_hash: function.code_sha256.clone(),
                last_used: Instant::now(),
            });
        }

        Ok(container_id)
    }

    /// Ensure the Docker image is available (pull if needed)
    async fn ensure_image(&self, image: &str) -> Result<(), DockerError> {
        // Check if image exists
        let output = Command::new("docker")
            .args(["image", "inspect", image])
            .output()
            .await?;

        if output.status.success() {
            return Ok(());
        }

        // Pull the image
        info!(image = %image, "Pulling Lambda base image");

        let output = Command::new("docker")
            .args(["pull", image])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DockerError::ImagePullFailed(format!(
                "Failed to pull {}: {}",
                image, stderr
            )));
        }

        Ok(())
    }

    /// Invoke a function using Docker
    pub async fn invoke(
        &self,
        function: &Function,
        code_path: &Path,
        payload: Bytes,
    ) -> Result<InvocationResult, DockerError> {
        if !self.docker_available {
            return Err(DockerError::NotAvailable(
                "Docker daemon is not running".to_string(),
            ));
        }

        let container_id = self.get_container(function, code_path).await?;
        let request_id = uuid::Uuid::new_v4().to_string();

        // Build deadline
        let deadline_ms =
            chrono::Utc::now().timestamp_millis() + (function.config.timeout as i64 * 1000);

        // Prepare event data - encode for passing to container
        let event_b64 = general_purpose::STANDARD.encode(&payload);

        // Execute via docker exec, passing event through stdin
        let timeout_secs = function.config.timeout as u64 + 5; // Add buffer

        debug!(
            container_id = %container_id,
            function = %function.config.function_name,
            "Invoking function in container"
        );

        // Start the container if not running
        let _ = Command::new("docker")
            .args(["start", &container_id])
            .output()
            .await;

        // For AWS Lambda containers, we need to call the Runtime Interface Client
        // The container entrypoint handles this, but we need to pass the event
        // We'll use docker exec to run a small script that posts to the local RIC

        let invoke_script = format!(
            r#"
import json
import sys
import base64

# Decode the event
event = json.loads(base64.b64decode('{}').decode('utf-8'))

# Import and call handler
handler_path = '{}'
module_path, handler_name = handler_path.rsplit('.', 1)
module_path = module_path.replace('/', '.')

# Simple context mock
class Context:
    function_name = '{}'
    function_version = '{}'
    invoked_function_arn = '{}'
    memory_limit_in_mb = {}
    aws_request_id = '{}'
    log_group_name = '/aws/lambda/{}'
    log_stream_name = '2024/01/01/[$LATEST]{}'
    
    def get_remaining_time_in_millis(self):
        return {}

try:
    import importlib
    sys.path.insert(0, '/var/task')
    module = importlib.import_module(module_path)
    handler = getattr(module, handler_name)
    result = handler(event, Context())
    print(json.dumps(result) if result is not None else 'null')
except Exception as e:
    import traceback
    print(json.dumps({{
        'errorMessage': str(e),
        'errorType': type(e).__name__,
        'stackTrace': traceback.format_exc().split('\\n')
    }}), file=sys.stderr)
    sys.exit(1)
"#,
            event_b64,
            function.config.handler,
            function.config.function_name,
            function.version,
            function.arn,
            function.config.memory_size,
            request_id,
            function.config.function_name,
            &request_id[..8],
            (deadline_ms - chrono::Utc::now().timestamp_millis()).max(0)
        );

        // Run the invocation
        let output = tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            Command::new("docker")
                .args([
                    "exec",
                    "-i",
                    &container_id,
                    "python3",
                    "-c",
                    &invoke_script,
                ])
                .output(),
        )
        .await;

        match output {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                if !stderr.is_empty() {
                    debug!(stderr = %stderr, "Container stderr");
                }

                if output.status.success() {
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
                    // Function error
                    let error_output = if !stderr.trim().is_empty() {
                        stderr.trim().to_string()
                    } else if !stdout.is_empty() {
                        stdout.to_string()
                    } else {
                        r#"{"errorMessage":"Unknown error","errorType":"Runtime.UnhandledError"}"#
                            .to_string()
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
            Ok(Err(e)) => {
                error!(error = %e, "Docker exec failed");
                Err(DockerError::ExecutionFailed(e.to_string()))
            }
            Err(_) => {
                warn!(
                    timeout = timeout_secs,
                    function = %function.config.function_name,
                    "Container execution timeout"
                );
                Err(DockerError::Timeout)
            }
        }
    }

    /// Cleanup all containers managed by this executor
    pub async fn cleanup(&self) {
        let containers = self.warm_containers.read().await;
        for container in containers.iter() {
            let _ = Command::new("docker")
                .args(["rm", "-f", &container.container_id])
                .output()
                .await;
        }
    }
}

impl Drop for DockerExecutor {
    fn drop(&mut self) {
        // Note: async cleanup happens in the cleanup task
        // This just ensures we try to clean up on shutdown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_image() {
        assert_eq!(
            runtime_image(&Runtime::Python312),
            "public.ecr.aws/lambda/python:3.12"
        );
        assert_eq!(
            runtime_image(&Runtime::Nodejs20),
            "public.ecr.aws/lambda/nodejs:20"
        );
    }

    #[test]
    fn test_executor_mode_from_str() {
        assert_eq!(
            ExecutorMode::from_str("docker"),
            Some(ExecutorMode::Docker)
        );
        assert_eq!(
            ExecutorMode::from_str("subprocess"),
            Some(ExecutorMode::Subprocess)
        );
        assert_eq!(ExecutorMode::from_str("auto"), Some(ExecutorMode::Auto));
        assert_eq!(ExecutorMode::from_str("invalid"), None);
    }
}
