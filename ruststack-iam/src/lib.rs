//! AWS IAM emulation for RustStack
//!
//! Provides stub IAM functionality for roles and policies.
//! This is a minimal implementation that allows tests to create roles
//! and policies without enforcing actual permissions.

pub mod handlers;
mod storage;

pub use handlers::handle_request;
pub use storage::{IamState, IamStorage};
