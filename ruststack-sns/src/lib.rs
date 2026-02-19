//! AWS SNS emulation for RustStack
//!
//! Provides in-memory pub/sub service with support for:
//! - CreateTopic, DeleteTopic, ListTopics
//! - Subscribe, Unsubscribe, ListSubscriptions
//! - Publish (push to subscribers)

pub mod handlers;
mod storage;

pub use handlers::handle_request;
pub use storage::{SnsState, SnsStorage};
