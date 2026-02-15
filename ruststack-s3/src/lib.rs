//! S3 implementation for RustStack
//!
//! This crate provides an S3-compatible object storage service.

pub mod handlers;
pub mod service;
pub mod storage;
pub mod xml;

pub use handlers::S3State;
pub use service::RustStackS3;
