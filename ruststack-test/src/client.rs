//! Client for interacting with RustStack services

use reqwest::Client;

/// Client for interacting with RustStack
pub struct RustStackClient {
    base_url: String,
    client: Client,
}

impl RustStackClient {
    /// Create a new client
    pub fn new(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { base_url, client }
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    // === S3 Operations ===

    /// Create an S3 bucket
    pub async fn create_bucket(&self, name: &str) -> Result<(), ClientError> {
        let url = format!("{}/{}", self.base_url, name);
        self.client
            .put(&url)
            .header("Content-Length", "0")
            .send()
            .await?;
        Ok(())
    }

    /// Put an object in S3
    pub async fn put_object(&self, bucket: &str, key: &str, body: &str) -> Result<(), ClientError> {
        let url = format!("{}/{}", self.base_url, bucket);
        let url = format!("{}/{}", url, key);
        self.client.put(&url).body(body.to_string()).send().await?;
        Ok(())
    }

    /// Get an object from S3
    pub async fn get_object(&self, bucket: &str, key: &str) -> Result<String, ClientError> {
        let url = format!("{}/{}", self.base_url, bucket);
        let url = format!("{}/{}", url, key);
        let response = self.client.get(&url).send().await?;
        Ok(response.text().await?)
    }

    // === SQS Operations ===

    /// Create an SQS queue
    pub async fn create_queue(&self, name: &str) -> Result<String, ClientError> {
        let url = format!("{}/", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("X-Amz-Target", "AmazonSQS.CreateQueue")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[("QueueName", name)])
            .send()
            .await?;

        let text = response.text().await?;
        // Extract queue URL from XML
        extract_xml_value(&text, "QueueUrl")
            .ok_or_else(|| ClientError::ParseError("Failed to parse queue URL".to_string()))
    }

    /// Send a message to an SQS queue
    pub async fn send_message(
        &self,
        queue_url: &str,
        message: &str,
    ) -> Result<String, ClientError> {
        let url = format!("{}/", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("X-Amz-Target", "AmazonSQS.SendMessage")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[("QueueUrl", queue_url), ("MessageBody", message)])
            .send()
            .await?;

        let text = response.text().await?;
        extract_xml_value(&text, "MessageId")
            .ok_or_else(|| ClientError::ParseError("Failed to parse message ID".to_string()))
    }

    /// Receive messages from an SQS queue
    pub async fn receive_messages(
        &self,
        queue_url: &str,
        max: i32,
    ) -> Result<Vec<SqsMessage>, ClientError> {
        let url = format!("{}/", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("X-Amz-Target", "AmazonSQS.ReceiveMessage")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[
                ("QueueUrl", queue_url),
                ("MaxNumberOfMessages", &max.to_string()),
            ])
            .send()
            .await?;

        let text = response.text().await?;
        // Simple XML parsing
        let mut messages = Vec::new();
        let mut pos = 0;
        while let Some(start) = text[pos..].find("<Message>") {
            pos = start + pos + 9;
            if let Some(body_start) = text[pos..].find("<Body>") {
                let body_start = body_start + 6;
                if let Some(body_end) = text[body_start..].find("</Body>") {
                    let body = text[body_start..body_start + body_end].to_string();
                    messages.push(SqsMessage { body });
                }
            }
        }
        Ok(messages)
    }

    // === SNS Operations ===

    /// Create an SNS topic
    pub async fn create_topic(&self, name: &str) -> Result<String, ClientError> {
        let url = format!("{}/", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("X-Amz-Target", "AmazonSNS.CreateTopic")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[("Name", name)])
            .send()
            .await?;

        let text = response.text().await?;
        extract_xml_value(&text, "TopicArn")
            .ok_or_else(|| ClientError::ParseError("Failed to parse topic ARN".to_string()))
    }

    /// Publish a message to an SNS topic
    pub async fn publish(&self, topic_arn: &str, message: &str) -> Result<String, ClientError> {
        let url = format!("{}/", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("X-Amz-Target", "AmazonSNS.Publish")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[("TopicArn", topic_arn), ("Message", message)])
            .send()
            .await?;

        let text = response.text().await?;
        extract_xml_value(&text, "MessageId")
            .ok_or_else(|| ClientError::ParseError("Failed to parse message ID".to_string()))
    }

    /// Subscribe to an SNS topic
    pub async fn subscribe(
        &self,
        topic_arn: &str,
        protocol: &str,
        endpoint: &str,
    ) -> Result<String, ClientError> {
        let url = format!("{}/", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("X-Amz-Target", "AmazonSNS.Subscribe")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[
                ("TopicArn", topic_arn),
                ("Protocol", protocol),
                ("Endpoint", endpoint),
            ])
            .send()
            .await?;

        let text = response.text().await?;
        extract_xml_value(&text, "SubscriptionArn")
            .ok_or_else(|| ClientError::ParseError("Failed to parse subscription ARN".to_string()))
    }
}

/// An SQS message
#[derive(Debug, Clone)]
pub struct SqsMessage {
    pub body: String,
}

/// Helper to extract a value from XML
fn extract_xml_value(xml: &str, tag: &str) -> Option<String> {
    let open_tag = format!("<{}>", tag);
    let close_tag = format!("</{}>", tag);

    if let Some(start) = xml.find(&open_tag) {
        let start = start + open_tag.len();
        if let Some(end) = xml[start..].find(&close_tag) {
            return Some(xml[start..start + end].to_string());
        }
    }
    None
}

/// Client errors
#[derive(Debug)]
pub enum ClientError {
    RequestError(reqwest::Error),
    ParseError(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::RequestError(e) => write!(f, "Request error: {}", e),
            ClientError::ParseError(e) => write!(f, "Parse error: {}", e),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<reqwest::Error> for ClientError {
    fn from(e: reqwest::Error) -> Self {
        ClientError::RequestError(e)
    }
}
