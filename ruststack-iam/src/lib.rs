//! AWS IAM emulation for RustStack
//!
//! Provides stub IAM functionality for roles and policies.
//! This is a minimal implementation that allows tests to create roles
//! and policies without enforcing actual permissions.
//!
//! ## IAM Policy Enforcement
//!
//! When `ENFORCE_IAM=1` is set, the policy engine will evaluate
//! access based on attached policies and return AccessDenied errors
//! when appropriate.

pub mod handlers;
pub mod middleware;
pub mod policy;
pub mod storage;

pub use handlers::handle_request;
pub use middleware::{access_denied_error, is_iam_enforced};
pub use policy::{Decision, EvaluationContext, PolicyDocument, PolicyEngine};
pub use storage::{IamState, IamStorage};
