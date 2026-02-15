//! Core types and traits for RustStack
//!
//! This crate provides common types used across all RustStack services.

pub mod account;
pub mod error;
pub mod request_id;

pub use account::{AccountRegionKey, StateStore};
pub use error::{AwsError, ErrorCode};
pub use request_id::RequestId;
