//! DynamoDB Local server management

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use thiserror::Error;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Failed to start DynamoDB Local: {0}")]
    StartFailed(String),

    #[error("DynamoDB Local JAR not found at {0}")]
    JarNotFound(PathBuf),

    #[error("Java not found in PATH")]
    JavaNotFound,

    #[error("Health check failed after {0} attempts")]
    HealthCheckFailed(u32),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// DynamoDB Local server manager
pub struct DynamoDBLocalServer {
    process: Option<Child>,
    port: u16,
    data_dir: Option<PathBuf>,
    jar_path: PathBuf,
}

impl DynamoDBLocalServer {
    /// Create a new server manager
    pub fn new(port: u16) -> Self {
        Self {
            process: None,
            port,
            data_dir: None,
            jar_path: PathBuf::from("DynamoDBLocal.jar"),
        }
    }

    /// Set the data directory for persistence
    pub fn with_data_dir(mut self, path: PathBuf) -> Self {
        self.data_dir = Some(path);
        self
    }

    /// Set the JAR path
    pub fn with_jar_path(mut self, path: PathBuf) -> Self {
        self.jar_path = path;
        self
    }

    /// Start DynamoDB Local
    pub async fn start(&mut self) -> Result<(), ServerError> {
        if self.process.is_some() {
            return Ok(()); // Already running
        }

        // Check Java is available
        if Command::new("java").arg("-version").output().is_err() {
            return Err(ServerError::JavaNotFound);
        }

        // Check JAR exists
        if !self.jar_path.exists() {
            return Err(ServerError::JarNotFound(self.jar_path.clone()));
        }

        let mut cmd = Command::new("java");
        cmd.arg("-Djava.library.path=./DynamoDBLocal_lib")
            .arg("-jar")
            .arg(&self.jar_path)
            .arg("-port")
            .arg(self.port.to_string())
            .arg("-sharedDb");

        if let Some(ref data_dir) = self.data_dir {
            cmd.arg("-dbPath").arg(data_dir);
        } else {
            cmd.arg("-inMemory");
        }

        cmd.stdout(Stdio::null()).stderr(Stdio::null());

        info!(port = self.port, "Starting DynamoDB Local");

        let child = cmd
            .spawn()
            .map_err(|e| ServerError::StartFailed(e.to_string()))?;

        self.process = Some(child);

        // Wait for server to be ready
        self.wait_for_ready().await?;

        info!(port = self.port, "DynamoDB Local started successfully");
        Ok(())
    }

    /// Wait for the server to be ready
    async fn wait_for_ready(&self) -> Result<(), ServerError> {
        let client = reqwest::Client::new();
        let url = format!("http://localhost:{}/", self.port);

        for attempt in 1..=30 {
            sleep(Duration::from_millis(100)).await;

            let result = client
                .post(&url)
                .header("X-Amz-Target", "DynamoDB_20120810.ListTables")
                .header("Content-Type", "application/x-amz-json-1.0")
                .body("{}")
                .send()
                .await;

            if result.is_ok() {
                return Ok(());
            }

            if attempt % 10 == 0 {
                warn!(attempt, "Still waiting for DynamoDB Local to start");
            }
        }

        Err(ServerError::HealthCheckFailed(30))
    }

    /// Stop DynamoDB Local
    pub fn stop(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
            let _ = process.wait();
            info!("DynamoDB Local stopped");
        }
    }

    /// Get the server URL
    pub fn url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    /// Check if server is running
    pub fn is_running(&self) -> bool {
        self.process.is_some()
    }
}

impl Drop for DynamoDBLocalServer {
    fn drop(&mut self) {
        self.stop();
    }
}
