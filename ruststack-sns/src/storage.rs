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

    pub fn publish(
        &self,
        topic_name: &str,
        _message: &str,
        _subject: Option<&str>,
    ) -> Result<String, SnsError> {
        let topic = self.get_topic(topic_name)?;

        let message_id = uuid::Uuid::new_v4().to_string();

        // Get subscriptions for this topic
        let subscriptions = self
            .subscriptions
            .get(topic_name)
            .map(|s| s.clone())
            .unwrap_or_default();

        // In a real implementation, would actually deliver to endpoints
        // For now, just log and return success
        for sub in &subscriptions {
            info!(topic = %topic_name, protocol = %sub.endpoint(), endpoint = %sub.endpoint(),
                message_id = %message_id, "Would publish to subscriber");
        }

        info!(topic = %topic_name, arn = %topic.arn, message_id = %message_id,
            subscriber_count = subscriptions.len(), "Published message");

        Ok(message_id)
    }
}

#[derive(Debug, Default)]
pub struct SnsState {
    storage: SnsStorage,
}

impl SnsState {
    pub fn new() -> Self {
        Self::default()
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
        _message: &str,
        _subject: Option<&str>,
    ) -> Result<String, SnsError> {
        self.storage.publish(topic_name, _message, _subject)
    }
}
