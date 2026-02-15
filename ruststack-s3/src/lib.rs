//! S3 implementation for RustStack
//!
//! This crate provides an S3-compatible object storage service built on top of s3s.

pub mod storage;
pub mod service;

pub use service::RustStackS3;
