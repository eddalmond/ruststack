//! AWS Secrets Manager emulation for RustStack
//!
//! Provides in-memory secrets storage with support for:
//! - CreateSecret, GetSecretValue, PutSecretValue
//! - DeleteSecret, DescribeSecret, ListSecrets
//! - Secret versioning (AWSCURRENT, AWSPREVIOUS)

pub mod handlers;
mod storage;

pub use handlers::handle_request;
pub use storage::{SecretsManagerState, SecretsManagerStorage};
