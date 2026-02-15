//! Lambda implementation for RustStack
//!
//! Provides Lambda function execution with Docker containers.

pub mod function;
pub mod invocation;
pub mod service;
pub mod runtime_api;

pub use service::LambdaService;
pub use function::{Function, FunctionConfig, Runtime};
