//! S3 implementation for RustStack
//!
//! This crate provides an S3-compatible object storage service.

pub mod storage;
pub mod service;
pub mod handlers;
pub mod xml;

pub use service::RustStackS3;
pub use handlers::S3State;
