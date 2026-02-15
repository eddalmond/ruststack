//! AWS authentication for RustStack
//!
//! Implements AWS Signature Version 4 and Version 2 for request authentication.

pub mod sigv4;

pub use sigv4::{SigV4Error, verify_signature};
