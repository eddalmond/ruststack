//! AWS API Gateway V2 (HTTP API) emulation for RustStack
//!
//! Provides basic API Gateway functionality for creating APIs
//! and integrations with Lambda functions.

pub mod handlers;
mod storage;

pub use handlers::handle_request;
pub use storage::{ApiGatewayState, ApiGatewayStorage};
