//! Test utilities for RustStack
//!
//! Provides utilities for integration testing with RustStack:
//! - Start/stop RustStack on a random port
//! - Reset state between tests
//! - Wait for services to be ready
//! - Client helpers for interacting with services
//!
//! ## Usage
//!
//! ```rust,no_run
//! use ruststack_test::{TestServer, Service};
//!
//! #[tokio::test]
//! async fn test_s3() {
//!     let server = TestServer::start().await.unwrap();
//!     
//!     // Use the server URL
//!     println!("Server running at: {}", server.url());
//!     
//!     // Reset state between tests
//!     server.reset().await;
//! }
//! ```

pub mod client;
pub mod server;

pub use client::RustStackClient;
pub use server::{Service, TestServer};

/// Default port for RustStack
pub const DEFAULT_PORT: u16 = 4566;

/// Timeout for waiting on services
pub const STARTUP_TIMEOUT_SECS: u64 = 30;
