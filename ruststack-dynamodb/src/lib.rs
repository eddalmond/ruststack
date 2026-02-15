//! DynamoDB implementation for RustStack
//!
//! This crate provides a DynamoDB-compatible API with in-memory storage.

pub mod expression;
pub mod handlers;
pub mod proxy;
pub mod server;
pub mod storage;

pub use expression::{ExpressionContext, ExpressionError};
pub use handlers::DynamoDBState;
pub use storage::{DynamoDBError, DynamoDBStorage};
