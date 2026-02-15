//! DynamoDB Local proxy

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DynamoDBError {
    #[error("DynamoDB Local not started")]
    NotStarted,

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),
}

/// Proxy for DynamoDB Local
pub struct DynamoDBProxy {
    port: u16,
    client: reqwest::Client,
}

impl Default for DynamoDBProxy {
    fn default() -> Self {
        Self::new(8000)
    }
}

impl DynamoDBProxy {
    /// Create a new proxy targeting DynamoDB Local on the given port
    pub fn new(port: u16) -> Self {
        Self {
            port,
            client: reqwest::Client::new(),
        }
    }

    /// Forward a request to DynamoDB Local
    pub async fn forward(
        &self,
        action: &str,
        body: bytes::Bytes,
    ) -> Result<bytes::Bytes, DynamoDBError> {
        let url = format!("http://localhost:{}/", self.port);

        let response = self.client
            .post(&url)
            .header("X-Amz-Target", format!("DynamoDB_20120810.{}", action))
            .header("Content-Type", "application/x-amz-json-1.0")
            .body(body)
            .send()
            .await?;

        Ok(response.bytes().await?)
    }
}

// TODO: Implement full proxy with pre/post processing
// - ARN transformation
// - Stream record generation
// - Error handling
