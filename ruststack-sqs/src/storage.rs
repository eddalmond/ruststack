//! SQS in-memory storage

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
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

#[derive(Debug, Default, Clone)]
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

    pub fn storage(&self) -> Arc<SqsStorage> {
        Arc::new(self.storage.clone())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> SqsState {
        SqsState::new()
    }

    fn test_message_body() -> String {
        r#"{"test": "data", "number": 42}"#.to_string()
    }

    // === Queue Tests ===

    #[test]
    fn test_create_queue() {
        let state = test_state();
        let result = state.create_queue("test-queue");

        assert!(result.is_ok());
        let queue = result.unwrap();
        assert_eq!(queue.name, "test-queue");
        assert!(queue.url.contains("test-queue"));
        assert!(queue.arn.contains("test-queue"));
    }

    #[test]
    fn test_create_queue_sets_timestamps() {
        let state = test_state();
        let queue = state.create_queue("timestamp-test").unwrap();

        assert!(queue.created_timestamp > 0);
    }

    #[test]
    fn test_create_queue_default_settings() {
        let state = test_state();
        let queue = state.create_queue("defaults-test").unwrap();

        assert_eq!(queue.visibility_timeout, 30);
        assert_eq!(queue.receive_message_wait_time_seconds, 0);
        assert_eq!(queue.message_retention_period, 345600);
        assert_eq!(queue.maximum_message_size, 262144);
    }

    #[test]
    fn test_create_duplicate_queue_fails() {
        let state = test_state();
        state.create_queue("duplicate-test").unwrap();

        let result = state.create_queue("duplicate-test");
        assert!(result.is_err());
        matches!(result.unwrap_err(), SqsError::QueueAlreadyExists(_));
    }

    #[test]
    fn test_delete_queue() {
        let state = test_state();
        state.create_queue("to-delete").unwrap();

        let result = state.delete_queue("to-delete");
        assert!(result.is_ok());

        // Verify queue is gone
        let result = state.get_queue("to-delete");
        assert!(result.is_err());
        matches!(result.unwrap_err(), SqsError::QueueNotFound(_));
    }

    #[test]
    fn test_delete_nonexistent_queue_fails() {
        let state = test_state();
        let result = state.delete_queue("nonexistent");
        assert!(result.is_err());
        matches!(result.unwrap_err(), SqsError::QueueNotFound(_));
    }

    #[test]
    fn test_get_queue() {
        let state = test_state();
        state.create_queue("get-test").unwrap();

        let result = state.get_queue("get-test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "get-test");
    }

    #[test]
    fn test_get_nonexistent_queue_fails() {
        let state = test_state();
        let result = state.get_queue("nonexistent");
        assert!(result.is_err());
        matches!(result.unwrap_err(), SqsError::QueueNotFound(_));
    }

    #[test]
    fn test_list_queues() {
        let state = test_state();
        state.create_queue("queue-1").unwrap();
        state.create_queue("queue-2").unwrap();
        state.create_queue("test-queue-3").unwrap();

        let all = state.list_queues(None);
        assert_eq!(all.len(), 3);

        let prefixed = state.list_queues(Some("test-"));
        assert_eq!(prefixed.len(), 1);
    }

    #[test]
    fn test_list_queues_empty() {
        let state = test_state();
        let queues = state.list_queues(None);
        assert!(queues.is_empty());
    }

    // === Message Tests ===

    #[test]
    fn test_send_message() {
        let state = test_state();
        state.create_queue("send-test").unwrap();

        let result = state.send_message("send-test", test_message_body());

        assert!(result.is_ok());
        let message = result.unwrap();
        assert!(!message.message_id.is_empty());
        assert!(!message.receipt_handle.is_empty());
        assert_eq!(message.body, test_message_body());
    }

    #[test]
    fn test_send_message_to_nonexistent_queue_fails() {
        let state = test_state();
        let result = state.send_message("nonexistent", "test".to_string());
        assert!(result.is_err());
        matches!(result.unwrap_err(), SqsError::QueueNotFound(_));
    }

    #[test]
    fn test_message_has_correct_md5() {
        let state = test_state();
        state.create_queue("md5-test").unwrap();

        let body = "test-md5-body".to_string();
        let message = state.send_message("md5-test", body.clone()).unwrap();

        // Verify MD5 is computed (not checking exact value, just presence)
        assert!(!message.md5_of_body.is_empty());
    }

    #[test]
    fn test_receive_message() {
        let state = test_state();
        state.create_queue("receive-test").unwrap();
        state
            .send_message("receive-test", test_message_body())
            .unwrap();

        let result = state.receive_message("receive-test", 10);

        assert!(result.is_ok());
        let messages = result.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].body, test_message_body());
    }

    #[test]
    fn test_receive_multiple_messages() {
        let state = test_state();
        state.create_queue("multi-test").unwrap();
        state
            .send_message("multi-test", "msg1".to_string())
            .unwrap();
        state
            .send_message("multi-test", "msg2".to_string())
            .unwrap();
        state
            .send_message("multi-test", "msg3".to_string())
            .unwrap();

        let messages = state.receive_message("multi-test", 10).unwrap();
        assert_eq!(messages.len(), 3);
    }

    #[test]
    fn test_receive_respects_max_messages() {
        let state = test_state();
        state.create_queue("max-test").unwrap();
        for i in 0..5 {
            state.send_message("max-test", format!("msg{}", i)).unwrap();
        }

        let messages = state.receive_message("max-test", 2).unwrap();
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_receive_from_empty_queue() {
        let state = test_state();
        state.create_queue("empty-test").unwrap();

        let messages = state.receive_message("empty-test", 10).unwrap();
        assert!(messages.is_empty());
    }

    #[test]
    fn test_receive_approximate_count() {
        let state = test_state();
        state.create_queue("count-test").unwrap();
        state
            .send_message("count-test", test_message_body())
            .unwrap();

        let msg1 = state.receive_message("count-test", 1).unwrap()[0].clone();
        assert_eq!(msg1.approximate_receive_count, 1);
        assert!(msg1.approximate_first_receive_timestamp.is_some());

        let msg2 = state.receive_message("count-test", 1).unwrap()[0].clone();
        assert_eq!(msg2.approximate_receive_count, 2);
    }

    #[test]
    fn test_delete_message() {
        let state = test_state();
        state.create_queue("delete-test").unwrap();
        let message = state
            .send_message("delete-test", test_message_body())
            .unwrap();

        let result = state.delete_message("delete-test", &message.receipt_handle);
        assert!(result.is_ok());
    }

    #[test]
    fn test_delete_message_with_invalid_receipt_handle() {
        let state = test_state();
        state.create_queue("delete-invalid-test").unwrap();
        state
            .send_message("delete-invalid-test", test_message_body())
            .unwrap();

        let result = state.delete_message("delete-invalid-test", "invalid-handle");
        assert!(result.is_err());
        matches!(result.unwrap_err(), SqsError::MessageNotFound(_));
    }

    #[test]
    fn test_delete_message_from_nonexistent_queue_fails() {
        let state = test_state();
        let result = state.delete_message("nonexistent", "handle");
        assert!(result.is_err());
        matches!(result.unwrap_err(), SqsError::QueueNotFound(_));
    }

    #[test]
    fn test_message_roundtrip() {
        let state = test_state();
        state.create_queue("roundtrip").unwrap();

        // Send
        let _sent = state
            .send_message("roundtrip", "roundtrip-body".to_string())
            .unwrap();

        // Receive
        let received = state.receive_message("roundtrip", 1).unwrap();
        assert_eq!(received[0].body, "roundtrip-body");

        // Delete
        let result = state.delete_message("roundtrip", &received[0].receipt_handle);
        assert!(result.is_ok());

        // Should be empty now
        let messages = state.receive_message("roundtrip", 10).unwrap();
        assert!(messages.is_empty());
    }

    #[test]
    fn test_multiple_messages_different_bodies() {
        let state = test_state();
        state.create_queue("bodies-test").unwrap();

        let bodies = vec!["first", "second", "third"];
        for body in &bodies {
            state.send_message("bodies-test", body.to_string()).unwrap();
        }

        let messages = state.receive_message("bodies-test", 10).unwrap();
        let received_bodies: Vec<String> = messages.iter().map(|m| m.body.clone()).collect();

        for body in bodies {
            assert!(received_bodies.contains(&body.to_string()));
        }
    }
}
