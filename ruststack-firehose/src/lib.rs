//! AWS Kinesis Firehose emulation for RustStack
//!
//! Provides delivery stream functionality for collecting and
//! buffering data. Supports S3 destination (stub).

pub mod handlers;
mod storage;

pub use handlers::handle_request;
pub use storage::{FirehoseState, FirehoseStorage};
