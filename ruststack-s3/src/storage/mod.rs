//! S3 storage backends

mod ephemeral;
mod traits;

#[cfg(test)]
mod tests;

pub use ephemeral::EphemeralStorage;
pub use traits::{
    ObjectStorage, StorageError, StoredObject, ObjectMetadata,
    PutObjectResult, DeleteResult, ListObjectsResult, ObjectSummary,
    PartInfo, CompletedPart, CompleteResult
};
