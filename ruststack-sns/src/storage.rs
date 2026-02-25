//! SNS in-memory storage

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum SnsError {
    #[error("Topic does not exist: {0}")]
    TopicNotFound(String),
    #[error("Topic already exists: {0}")]
    TopicAlreadyExists(String),
    #[error("Subscription not found: {0}")]
    SubscriptionNotFound(String),
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    pub name: String,
    pub arn: String,
    pub topic_arn: String,
    pub created_timestamp: i64,
}

impl Topic {
    pub fn new(name: String) -> Self {
        let now = chrono::Utc::now().timestamp();
        // let uuid = uuid::Uuid::new_v4().to_string()[..8].to_string();
        let topic_arn = format!("arn:aws:sns:us-east-1:000000000000:{}", name);
        Self {
            name,
            arn: topic_arn.clone(),
            topic_arn,
            created_timestamp: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "protocol")]
pub enum Subscription {
    #[serde(rename = "sqs")]
    Sqs {
        endpoint: String,
        subscription_arn: String,
    },
    #[serde(rename = "http")]
    Http {
        endpoint: String,
        subscription_arn: String,
    },
    #[serde(rename = "https")]
    Https {
        endpoint: String,
        subscription_arn: String,
    },
    #[serde(rename = "email")]
    Email {
        endpoint: String,
        subscription_arn: String,
    },
    #[serde(rename = "lambda")]
    Lambda {
        endpoint: String,
        subscription_arn: String,
    },
}

impl Subscription {
    pub fn new(protocol: &str, endpoint: &str) -> Self {
        let short_uuid = uuid::Uuid::new_v4().to_string();
        let short_uuid = &short_uuid[..8];
        let subscription_arn = format!(
            "arn:aws:sns:us-east-1:000000000000:{}:{}",
            uuid::Uuid::new_v4(),
            short_uuid
        );

        match protocol.to_lowercase().as_str() {
            "sqs" => Subscription::Sqs {
                endpoint: endpoint.to_string(),
                subscription_arn,
            },
            "http" => Subscription::Http {
                endpoint: endpoint.to_string(),
                subscription_arn,
            },
            "https" => Subscription::Https {
                endpoint: endpoint.to_string(),
                subscription_arn,
            },
            "email" => Subscription::Email {
                endpoint: endpoint.to_string(),
                subscription_arn,
            },
            "lambda" => Subscription::Lambda {
                endpoint: endpoint.to_string(),
                subscription_arn,
            },
            _ => Subscription::Sqs {
                endpoint: endpoint.to_string(),
                subscription_arn,
            },
        }
    }

    pub fn endpoint(&self) -> &str {
        match self {
            Subscription::Sqs { endpoint, .. } => endpoint,
            Subscription::Http { endpoint, .. } => endpoint,
            Subscription::Https { endpoint, .. } => endpoint,
            Subscription::Email { endpoint, .. } => endpoint,
            Subscription::Lambda { endpoint, .. } => endpoint,
        }
    }

    pub fn arn(&self) -> &str {
        match self {
            Subscription::Sqs {
                subscription_arn, ..
            } => subscription_arn,
            Subscription::Http {
                subscription_arn, ..
            } => subscription_arn,
            Subscription::Https {
                subscription_arn, ..
            } => subscription_arn,
            Subscription::Email {
                subscription_arn, ..
            } => subscription_arn,
            Subscription::Lambda {
                subscription_arn, ..
            } => subscription_arn,
        }
    }
}

#[derive(Debug, Default)]
pub struct SnsStorage {
    topics: DashMap<String, Topic>,
    subscriptions: DashMap<String, Vec<Subscription>>,
}

impl SnsStorage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_topic(&self, name: &str) -> Result<Topic, SnsError> {
        if self.topics.contains_key(name) {
            return Err(SnsError::TopicAlreadyExists(name.to_string()));
        }

        let topic = Topic::new(name.to_string());
        info!(name = %name, arn = %topic.arn, "Creating topic");
        self.topics.insert(name.to_string(), topic.clone());
        self.subscriptions.insert(name.to_string(), Vec::new());
        Ok(topic)
    }

    pub fn delete_topic(&self, name: &str) -> Result<(), SnsError> {
        if !self.topics.contains_key(name) {
            return Err(SnsError::TopicNotFound(name.to_string()));
        }

        info!(name = %name, "Deleting topic");
        self.topics.remove(name);
        self.subscriptions.remove(name);
        Ok(())
    }

    pub fn get_topic(&self, name: &str) -> Result<Topic, SnsError> {
        self.topics
            .get(name)
            .map(|t| t.clone())
            .ok_or_else(|| SnsError::TopicNotFound(name.to_string()))
    }

    pub fn list_topics(&self) -> Vec<Topic> {
        self.topics.iter().map(|t| t.value().clone()).collect()
    }

    pub fn subscribe(
        &self,
        topic_name: &str,
        protocol: &str,
        endpoint: &str,
    ) -> Result<Subscription, SnsError> {
        // Verify topic exists
        let _ = self.get_topic(topic_name)?;

        let subscription = Subscription::new(protocol, endpoint);

        if let Some(mut subs) = self.subscriptions.get_mut(topic_name) {
            subs.push(subscription.clone());
        }

        info!(topic = %topic_name, protocol = %protocol, endpoint = %endpoint, "Subscribed");
        Ok(subscription)
    }

    pub fn unsubscribe(&self, subscription_arn: &str) -> Result<(), SnsError> {
        for mut subs in self.subscriptions.iter_mut() {
            let original_len = subs.len();
            subs.retain(|s| s.arn() != subscription_arn);
            if subs.len() != original_len {
                info!(arn = %subscription_arn, "Unsubscribed");
                return Ok(());
            }
        }

        Err(SnsError::SubscriptionNotFound(subscription_arn.to_string()))
    }

    pub fn list_subscriptions(&self, topic_name: &str) -> Result<Vec<Subscription>, SnsError> {
        let _ = self.get_topic(topic_name)?;

        Ok(self
            .subscriptions
            .get(topic_name)
            .map(|s| s.clone())
            .unwrap_or_default())
    }

    pub fn list_all_subscriptions(&self) -> Vec<(String, Subscription)> {
        let mut result = Vec::new();
        for subs in self.subscriptions.iter() {
            for sub in subs.value() {
                result.push((subs.key().clone(), sub.clone()));
            }
        }
        result
    }
}

use std::sync::Arc;

#[derive(Default)]
#[allow(clippy::type_complexity)]
pub struct SnsState {
    storage: SnsStorage,
    sqs_fanout: Option<Arc<dyn Fn(&str, &str) + Send + Sync>>,
}

impl SnsState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a fan-out callback for SQS
    /// The callback receives (queue_name, message_body)
    pub fn set_sqs_fanout<F>(&mut self, callback: F)
    where
        F: Fn(&str, &str) + Send + Sync + 'static,
    {
        self.sqs_fanout = Some(Arc::new(callback));
    }

    pub fn create_topic(&self, name: &str) -> Result<Topic, SnsError> {
        self.storage.create_topic(name)
    }

    pub fn delete_topic(&self, name: &str) -> Result<(), SnsError> {
        self.storage.delete_topic(name)
    }

    pub fn get_topic(&self, name: &str) -> Result<Topic, SnsError> {
        self.storage.get_topic(name)
    }

    pub fn list_topics(&self) -> Vec<Topic> {
        self.storage.list_topics()
    }

    pub fn subscribe(
        &self,
        topic_name: &str,
        protocol: &str,
        endpoint: &str,
    ) -> Result<Subscription, SnsError> {
        self.storage.subscribe(topic_name, protocol, endpoint)
    }

    pub fn unsubscribe(&self, subscription_arn: &str) -> Result<(), SnsError> {
        self.storage.unsubscribe(subscription_arn)
    }

    pub fn list_subscriptions(&self, topic_name: &str) -> Result<Vec<Subscription>, SnsError> {
        self.storage.list_subscriptions(topic_name)
    }

    pub fn list_all_subscriptions(&self) -> Vec<(String, Subscription)> {
        self.storage.list_all_subscriptions()
    }

    pub fn publish(
        &self,
        topic_name: &str,
        message: &str,
        subject: Option<&str>,
    ) -> Result<String, SnsError> {
        let topic = self.storage.get_topic(topic_name)?;

        let message_id = uuid::Uuid::new_v4().to_string();

        // Get subscriptions
        let subscriptions = self
            .storage
            .subscriptions
            .get(topic_name)
            .map(|s| s.clone())
            .unwrap_or_default();

        // Fan out to subscribers
        for sub in &subscriptions {
            match sub {
                Subscription::Sqs { endpoint, .. } => {
                    if let Some(ref callback) = self.sqs_fanout {
                        let queue_name = endpoint
                            .strip_prefix("http://localhost:4566/")
                            .or_else(|| endpoint.strip_prefix("http://127.0.0.1:4566/"))
                            .unwrap_or(endpoint);

                        let sns_message = serde_json::json!({
                            "Type": "Notification",
                            "MessageId": message_id,
                            "TopicArn": topic.arn,
                            "Subject": subject,
                            "Message": message,
                            "Timestamp": chrono::Utc::now().to_rfc3339(),
                        });

                        callback(queue_name, &sns_message.to_string());
                        info!(topic = %topic_name, queue = %queue_name, "Published to SQS");
                    }
                }
                Subscription::Lambda { endpoint, .. } => {
                    info!(topic = %topic_name, lambda = %endpoint,
                        message_id = %message_id, "Would publish to Lambda");
                }
                Subscription::Http { endpoint, .. } | Subscription::Https { endpoint, .. } => {
                    info!(topic = %topic_name, http = %endpoint,
                        message_id = %message_id, "Would publish to HTTP");
                }
                Subscription::Email { endpoint, .. } => {
                    info!(topic = %topic_name, email = %endpoint,
                        message_id = %message_id, "Would publish to Email");
                }
            }
        }

        info!(topic = %topic_name, arn = %topic.arn, message_id = %message_id,
            subscriber_count = subscriptions.len(), "Published message");

        Ok(message_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> SnsState {
        SnsState::new()
    }

    // === Topic Tests ===

    #[test]
    fn test_create_topic() {
        let state = test_state();
        let result = state.create_topic("test-topic");
        
        assert!(result.is_ok());
        let topic = result.unwrap();
        assert_eq!(topic.name, "test-topic");
        assert!(topic.arn.contains("test-topic"));
        assert!(topic.topic_arn.contains("test-topic"));
    }

    #[test]
    fn test_create_topic_sets_timestamps() {
        let state = test_state();
        let topic = state.create_topic("timestamp-test").unwrap();
        
        assert!(topic.created_timestamp > 0);
    }

    #[test]
    fn test_create_duplicate_topic_fails() {
        let state = test_state();
        state.create_topic("duplicate-test").unwrap();
        
        let result = state.create_topic("duplicate-test");
        assert!(result.is_err());
        matches!(result.unwrap_err(), SnsError::TopicAlreadyExists(_));
    }

    #[test]
    fn test_delete_topic() {
        let state = test_state();
        state.create_topic("to-delete").unwrap();
        
        let result = state.delete_topic("to-delete");
        assert!(result.is_ok());
        
        // Verify topic is gone
        let result = state.get_topic("to-delete");
        assert!(result.is_err());
        matches!(result.unwrap_err(), SnsError::TopicNotFound(_));
    }

    #[test]
    fn test_delete_nonexistent_topic_fails() {
        let state = test_state();
        let result = state.delete_topic("nonexistent");
        assert!(result.is_err());
        matches!(result.unwrap_err(), SnsError::TopicNotFound(_));
    }

    #[test]
    fn test_get_topic() {
        let state = test_state();
        state.create_topic("get-test").unwrap();
        
        let result = state.get_topic("get-test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "get-test");
    }

    #[test]
    fn test_get_nonexistent_topic_fails() {
        let state = test_state();
        let result = state.get_topic("nonexistent");
        assert!(result.is_err());
        matches!(result.unwrap_err(), SnsError::TopicNotFound(_));
    }

    #[test]
    fn test_list_topics() {
        let state = test_state();
        state.create_topic("topic-1").unwrap();
        state.create_topic("topic-2").unwrap();
        
        let topics = state.list_topics();
        assert_eq!(topics.len(), 2);
    }

    #[test]
    fn test_list_topics_empty() {
        let state = test_state();
        let topics = state.list_topics();
        assert!(topics.is_empty());
    }

    // === Subscription Tests ===

    #[test]
    fn test_subscribe_sqs() {
        let state = test_state();
        state.create_topic("sqs-topic").unwrap();
        
        let result = state.subscribe("sqs-topic", "sqs", "http://localhost:4566/my-queue");
        
        assert!(result.is_ok());
        let sub = result.unwrap();
        matches!(sub, Subscription::Sqs { .. });
    }

    #[test]
    fn test_subscribe_lambda() {
        let state = test_state();
        state.create_topic("lambda-topic").unwrap();
        
        let result = state.subscribe("lambda-topic", "lambda", "arn:aws:lambda:us-east-1:000000000000:function:my-function");
        
        assert!(result.is_ok());
        let sub = result.unwrap();
        matches!(sub, Subscription::Lambda { .. });
    }

    #[test]
    fn test_subscribe_http() {
        let state = test_state();
        state.create_topic("http-topic").unwrap();
        
        let result = state.subscribe("http-topic", "http", "https://example.com/webhook");
        
        assert!(result.is_ok());
        let sub = result.unwrap();
        matches!(sub, Subscription::Http { .. });
    }

    #[test]
    fn test_subscribe_https() {
        let state = test_state();
        state.create_topic("https-topic").unwrap();
        
        let result = state.subscribe("https-topic", "https", "https://secure.example.com/hook");
        
        assert!(result.is_ok());
        let sub = result.unwrap();
        matches!(sub, Subscription::Https { .. });
    }

    #[test]
    fn test_subscribe_email() {
        let state = test_state();
        state.create_topic("email-topic").unwrap();
        
        let result = state.subscribe("email-topic", "email", "test@example.com");
        
        assert!(result.is_ok());
        let sub = result.unwrap();
        matches!(sub, Subscription::Email { .. });
    }

    #[test]
    fn test_subscribe_to_nonexistent_topic_fails() {
        let state = test_state();
        let result = state.subscribe("nonexistent", "sqs", "http://localhost:4566/queue");
        assert!(result.is_err());
        matches!(result.unwrap_err(), SnsError::TopicNotFound(_));
    }

    #[test]
    fn test_subscription_has_endpoint() {
        let state = test_state();
        state.create_topic("endpoint-test").unwrap();
        
        let sub = state.subscribe("endpoint-test", "lambda", "arn:aws:lambda:us-east-1:123456789012:function:Test").unwrap();
        
        assert_eq!(sub.endpoint(), "arn:aws:lambda:us-east-1:123456789012:function:Test");
    }

    #[test]
    fn test_subscription_has_arn() {
        let state = test_state();
        state.create_topic("arn-test").unwrap();
        
        let sub = state.subscribe("arn-test", "sqs", "http://localhost:4566/my-queue").unwrap();
        
        assert!(!sub.arn().is_empty());
        assert!(sub.arn().contains("arn:aws:sns"));
    }

    #[test]
    fn test_list_subscriptions() {
        let state = test_state();
        state.create_topic("list-test").unwrap();
        state.subscribe("list-test", "sqs", "http://localhost:4566/queue1").unwrap();
        state.subscribe("list-test", "lambda", "arn:aws:lambda:us-east-1:000000000000:function:fn").unwrap();
        
        let subs = state.list_subscriptions("list-test").unwrap();
        assert_eq!(subs.len(), 2);
    }

    #[test]
    fn test_list_all_subscriptions() {
        let state = test_state();
        state.create_topic("all-test-1").unwrap();
        state.create_topic("all-test-2").unwrap();
        state.subscribe("all-test-1", "sqs", "http://localhost:4566/q1").unwrap();
        state.subscribe("all-test-2", "lambda", "arn:aws:lambda:us-east-1:000000000000:function:fn").unwrap();
        
        let all = state.list_all_subscriptions();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_unsubscribe() {
        let state = test_state();
        state.create_topic("unsub-test").unwrap();
        
        let sub = state.subscribe("unsub-test", "sqs", "http://localhost:4566/queue").unwrap();
        let arn = sub.arn().to_string();
        
        let result = state.unsubscribe(&arn);
        assert!(result.is_ok());
        
        let subs = state.list_subscriptions("unsub-test").unwrap();
        assert!(subs.is_empty());
    }

    #[test]
    fn test_unsubscribe_invalid_arn_fails() {
        let state = test_state();
        state.create_topic("unsub-invalid-test").unwrap();
        
        let result = state.unsubscribe("arn:aws:sns:us-east-1:000000000000:invalid:12345678");
        assert!(result.is_err());
        matches!(result.unwrap_err(), SnsError::SubscriptionNotFound(_));
    }

    // === Publish Tests ===

    #[test]
    fn test_publish() {
        let state = test_state();
        state.create_topic("publish-test").unwrap();
        
        let result = state.publish("publish-test", "test message", None);
        
        assert!(result.is_ok());
        let message_id = result.unwrap();
        assert!(!message_id.is_empty());
    }

    #[test]
    fn test_publish_with_subject() {
        let state = test_state();
        state.create_topic("subject-test").unwrap();
        
        let result = state.publish("subject-test", "message body", Some("Test Subject"));
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_publish_to_nonexistent_topic_fails() {
        let state = test_state();
        let result = state.publish("nonexistent", "test", None);
        assert!(result.is_err());
        matches!(result.unwrap_err(), SnsError::TopicNotFound(_));
    }

    #[test]
    fn test_publish_returns_message_id() {
        let state = test_state();
        state.create_topic("msgid-test").unwrap();
        
        let msg_id = state.publish("msgid-test", "test", None).unwrap();
        
        // Should be a valid UUID
        assert!(uuid::Uuid::parse_str(&msg_id).is_ok());
    }

    #[test]
    fn test_topic_arn_format() {
        let state = test_state();
        state.create_topic("arn-format-test").unwrap();
        
        let topic = state.get_topic("arn-format-test").unwrap();
        
        assert!(topic.arn.starts_with("arn:aws:sns:us-east-1:000000000000:arn-format-test"));
    }
}
