//! Lambda implementation for RustStack
//!
//! Provides Lambda function execution with subprocess-based Python runtime
//! or Docker-based isolated execution.

pub mod docker;
pub mod function;
pub mod handlers;
pub mod invocation;
pub mod runtime_api;
pub mod service;

pub use docker::{DockerExecutor, DockerExecutorConfig, ExecutorMode};
pub use function::{Function, FunctionConfig, Runtime};
pub use handlers::LambdaState;
pub use service::LambdaService;
