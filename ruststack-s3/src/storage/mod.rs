//! S3 storage backends

mod ephemeral;
mod traits;

pub use ephemeral::EphemeralStorage;
pub use traits::{
    ObjectStorage, StorageError, StoredObject, ObjectMetadata,
    PutObjectResult, DeleteResult, ListObjectsResult, ObjectSummary,
    PartInfo, CompletedPart, CompleteResult
};
