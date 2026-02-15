//! Lambda implementation for RustStack
//!
//! Provides Lambda function execution with subprocess-based Python runtime.

pub mod function;
pub mod handlers;
pub mod invocation;
pub mod runtime_api;
pub mod service;

pub use function::{Function, FunctionConfig, Runtime};
pub use handlers::LambdaState;
pub use service::LambdaService;
