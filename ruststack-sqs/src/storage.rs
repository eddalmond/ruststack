//! SQS in-memory storage

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum SqsError {
    #[error("Queue does not exist: {0}")]
    QueueNotFound(String),
    #[error("Queue already exists: {0}")]
    QueueAlreadyExists(String),
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
    #[error("Message not found: {0}")]
    MessageNotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Queue {
    pub name: String,
    pub url: String,
    pub arn: String,
    pub created_timestamp: i64,
    pub visibility_timeout: i32,
    pub receive_message_wait_time_seconds: i32,
    pub message_retention_period: i32,
    pub maximum_message_size: i32,
}

impl Queue {
    pub fn new(name: String) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            url: format!("http://localhost:4566/000000000000/{}", name),
            arn: format!("arn:aws:sqs:us-east-1:000000000000:{}", name),
            name,
            created_timestamp: now,
            visibility_timeout: 30,
            receive_message_wait_time_seconds: 0,
            message_retention_period: 345600, // 4 days
            maximum_message_size: 262144,     // 256KB
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub message_id: String,
    pub receipt_handle: String,
    pub body: String,
    pub md5_of_body: String,
    pub attribute_names: Vec<String>,
    pub message_attributes: std::collections::HashMap<String, String>,
    pub sent_timestamp: i64,
    pub approximate_receive_count: i32,
    pub approximate_first_receive_timestamp: Option<i64>,
}

impl Message {
    pub fn new(body: String) -> Self {
        use uuid::Uuid;

        let message_id = Uuid::new_v4().to_string();
        let receipt_handle = Uuid::new_v4().to_string();

        Self {
            message_id,
            receipt_handle,
            body: body.clone(),
            md5_of_body: format!("{:x}", simple_md5(&body)),
            attribute_names: vec![],
            message_attributes: std::collections::HashMap::new(),
            sent_timestamp: chrono::Utc::now().timestamp_millis(),
            approximate_receive_count: 0,
            approximate_first_receive_timestamp: None,
        }
    }
}

fn simple_md5(input: &str) -> u128 {
    // Simple hash for MD5 simulation
    let mut hash: u128 = 0;
    for (i, byte) in input.bytes().enumerate() {
        hash = hash.wrapping_add((byte as u128).wrapping_mul((i as u128).wrapping_add(1)));
        hash = hash.rotate_left(5);
    }
    hash
}

#[derive(Debug, Default)]
pub struct SqsStorage {
    queues: DashMap<String, Queue>,
    messages: DashMap<String, VecDeque<Message>>,
}

impl SqsStorage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_queue(&self, name: &str) -> Result<Queue, SqsError> {
        if self.queues.contains_key(name) {
            return Err(SqsError::QueueAlreadyExists(name.to_string()));
        }

        let queue = Queue::new(name.to_string());
        info!(name = %name, url = %queue.url, "Creating queue");
        self.queues.insert(name.to_string(), queue.clone());
        self.messages.insert(name.to_string(), VecDeque::new());
        Ok(queue)
    }

    pub fn delete_queue(&self, name: &str) -> Result<(), SqsError> {
        if !self.queues.contains_key(name) {
            return Err(SqsError::QueueNotFound(name.to_string()));
        }

        info!(name = %name, "Deleting queue");
        self.queues.remove(name);
        self.messages.remove(name);
        Ok(())
    }

    pub fn get_queue(&self, name: &str) -> Result<Queue, SqsError> {
        self.queues
            .get(name)
            .map(|q| q.value().clone())
            .ok_or_else(|| SqsError::QueueNotFound(name.to_string()))
    }

    pub fn list_queues(&self, prefix: Option<&str>) -> Vec<String> {
        self.queues
            .iter()
            .filter(|q| {
                if let Some(p) = prefix {
                    q.key().starts_with(p)
                } else {
                    true
                }
            })
            .map(|q| q.value().url.clone())
            .collect()
    }

    pub fn send_message(&self, queue_name: &str, body: String) -> Result<Message, SqsError> {
        // Verify queue exists
        let _ = self.get_queue(queue_name)?;

        let message = Message::new(body);

        if let Some(mut msgs) = self.messages.get_mut(queue_name) {
            msgs.push_back(message.clone());
        }

        info!(queue = %queue_name, message_id = %message.message_id, "Sent message");
        Ok(message)
    }

    pub fn receive_message(
        &self,
        queue_name: &str,
        max_messages: i32,
    ) -> Result<Vec<Message>, SqsError> {
        // Verify queue exists
        let _ = self.get_queue(queue_name)?;

        let mut result = Vec::new();
        let max = max_messages.clamp(1, 10) as usize;

        if let Some(mut messages) = self.messages.get_mut(queue_name) {
            for _ in 0..max {
                if let Some(mut msg) = messages.pop_front() {
                    msg.approximate_receive_count += 1;
                    if msg.approximate_first_receive_timestamp.is_none() {
                        msg.approximate_first_receive_timestamp =
                            Some(chrono::Utc::now().timestamp_millis());
                    }
                    // Generate new receipt handle for next receive
                    msg.receipt_handle = uuid::Uuid::new_v4().to_string();
                    result.push(msg);
                }
            }

            // Re-queue any messages (simplified - in real impl, would handle visibility timeout)
            for msg in &result {
                messages.push_back(msg.clone());
            }
        }

        info!(queue = %queue_name, count = result.len(), "Received messages");
        Ok(result)
    }

    pub fn delete_message(&self, queue_name: &str, receipt_handle: &str) -> Result<(), SqsError> {
        let mut messages = self
            .messages
            .get_mut(queue_name)
            .ok_or_else(|| SqsError::QueueNotFound(queue_name.to_string()))?;

        let original_len = messages.len();
        messages.retain(|m| m.receipt_handle != receipt_handle);

        if messages.len() == original_len {
            return Err(SqsError::MessageNotFound(receipt_handle.to_string()));
        }

        info!(queue = %queue_name, receipt = %receipt_handle, "Deleted message");
        Ok(())
    }
}

/// State for SQS handlers
#[derive(Debug, Default)]
pub struct SqsState {
    storage: SqsStorage,
}

impl SqsState {
    pub fn new() -> Self {
        Self::default()
    }

    // Delegate methods to storage
    pub fn create_queue(&self, name: &str) -> Result<Queue, SqsError> {
        self.storage.create_queue(name)
    }

    pub fn delete_queue(&self, name: &str) -> Result<(), SqsError> {
        self.storage.delete_queue(name)
    }

    pub fn get_queue(&self, name: &str) -> Result<Queue, SqsError> {
        self.storage.get_queue(name)
    }

    pub fn list_queues(&self, prefix: Option<&str>) -> Vec<String> {
        self.storage.list_queues(prefix)
    }

    pub fn send_message(&self, queue_name: &str, body: String) -> Result<Message, SqsError> {
        self.storage.send_message(queue_name, body)
    }

    pub fn receive_message(
        &self,
        queue_name: &str,
        max_messages: i32,
    ) -> Result<Vec<Message>, SqsError> {
        self.storage.receive_message(queue_name, max_messages)
    }

    pub fn delete_message(&self, queue_name: &str, receipt_handle: &str) -> Result<(), SqsError> {
        self.storage.delete_message(queue_name, receipt_handle)
    }
}
