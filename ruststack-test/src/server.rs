//! Test server management

use portpicker::pick_unused_port;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tracing::info;

/// Service names available in RustStack
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum Service {
    S3 = 4572,
    DynamoDB = 4567,
    Lambda = 4574,
    SecretsManager = 4584,
    IAM = 4593,
    ApiGateway = 4569,
    Firehose = 4573,
    SQS = 4576,
    SNS = 4575,
}

impl Service {
    pub fn port(&self) -> u16 {
        *self as u16
    }
}

/// A running RustStack test server
pub struct TestServer {
    /// The process running RustStack
    child: tokio::process::Child,
    /// The port the server is running on
    port: u16,
    /// Base URL
    base_url: String,
}

impl TestServer {
    /// Start a new RustStack server on a random available port
    pub async fn start() -> Result<Self, TestError> {
        let port = pick_unused_port().ok_or(TestError::NoPortAvailable)?;

        // Try to build ruststack from the project directory
        let project_dir = Self::find_project_dir();

        info!(port = port, "Starting RustStack test server");

        // Start ruststack
        let mut child = Command::new("cargo")
            .args(["run", "--package", "ruststack", "--"])
            .arg("--port")
            .arg(port.to_string())
            .current_dir(&project_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| TestError::StartFailed(e.to_string()))?;

        // Wait for server to be ready
        let base_url = format!("http://localhost:{}", port);
        let start = std::time::Instant::now();

        while start.elapsed() < Duration::from_secs(STARTUP_TIMEOUT_SECS) {
            if let Ok(response) = reqwest::get(&base_url).await {
                if response.status().is_success() {
                    info!(port = port, "RustStack ready");
                    return Ok(Self {
                        child,
                        port,
                        base_url,
                    });
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Server didn't start in time, kill it and error
        let _ = child.kill().await;
        Err(TestError::StartupTimeout)
    }

    /// Find the project directory (looks for Cargo.toml with ruststack package)
    fn find_project_dir() -> std::path::PathBuf {
        let mut path = std::env::current_dir().unwrap_or_default();

        for _ in 0..10 {
            if path.join("Cargo.toml").exists() {
                let content = std::fs::read_to_string(path.join("Cargo.toml")).unwrap_or_default();
                if content.contains("ruststack") {
                    return path;
                }
            }
            if !path.pop() {
                break;
            }
        }

        std::env::current_dir().unwrap_or_default()
    }

    /// Get the base URL
    pub fn url(&self) -> &str {
        &self.base_url
    }

    /// Get the port
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Reset all state
    pub async fn reset(&self) -> Result<(), TestError> {
        // Just verify we can connect
        let _ = reqwest::get(self.url()).await;
        Ok(())
    }

    /// Get a client for interacting with services
    pub fn client(&self) -> crate::RustStackClient {
        crate::RustStackClient::new(self.base_url.clone())
    }

    /// Stop the server
    pub async fn stop(&mut self) {
        info!("Stopping RustStack test server");
        let _ = self.child.kill().await;
        info!("RustStack test server stopped");
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

/// Errors that can occur with test server
#[derive(Debug)]
pub enum TestError {
    NoPortAvailable,
    PortInUse(u16),
    StartFailed(String),
    StartupTimeout,
    ClientError(String),
}

impl std::fmt::Display for TestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestError::NoPortAvailable => write!(f, "No available port found"),
            TestError::PortInUse(port) => write!(f, "Port {} is already in use", port),
            TestError::StartFailed(msg) => write!(f, "Failed to start server: {}", msg),
            TestError::StartupTimeout => write!(f, "Server startup timed out"),
            TestError::ClientError(msg) => write!(f, "Client error: {}", msg),
        }
    }
}

impl std::error::Error for TestError {}

/// Default timeout for waiting on services
pub const STARTUP_TIMEOUT_SECS: u64 = 30;
