//! DynamoDB implementation for RustStack
//!
//! This crate provides a DynamoDB-compatible API, proxying to DynamoDB Local.

pub mod proxy;
pub mod server;

pub use proxy::DynamoDBProxy;
