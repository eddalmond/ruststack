//! S3 service implementation

use crate::storage::{EphemeralStorage, ObjectStorage};
use std::sync::Arc;

/// RustStack S3 service
pub struct RustStackS3 {
    storage: Arc<dyn ObjectStorage>,
}

impl Default for RustStackS3 {
    fn default() -> Self {
        Self::new()
    }
}

impl RustStackS3 {
    /// Create a new S3 service with ephemeral storage
    pub fn new() -> Self {
        Self {
            storage: Arc::new(EphemeralStorage::new()),
        }
    }

    /// Create a new S3 service with custom storage backend
    pub fn with_storage(storage: Arc<dyn ObjectStorage>) -> Self {
        Self { storage }
    }

    /// Get reference to storage backend
    pub fn storage(&self) -> &Arc<dyn ObjectStorage> {
        &self.storage
    }
}

// TODO: Implement s3s::S3 trait for RustStackS3
// This will be done in Phase 2 when we integrate with s3s framework
