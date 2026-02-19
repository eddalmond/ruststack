//! AWS SQS emulation for RustStack
//!
//! Provides in-memory queue service with support for:
//! - CreateQueue, DeleteQueue, ListQueues
//! - SendMessage, ReceiveMessage, DeleteMessage
//! - Queue attributes (visibility timeout, etc.)

pub mod handlers;
mod storage;

pub use handlers::handle_request;
pub use storage::{SqsState, SqsStorage};
