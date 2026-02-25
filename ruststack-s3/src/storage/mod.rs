//! S3 storage backends

mod ephemeral;
mod persistent;
mod traits;

#[cfg(test)]
mod tests;

pub use ephemeral::EphemeralStorage;
pub use persistent::PersistentStorage;
pub use traits::{
    CompleteResult, CompletedPart, DeleteResult, ListObjectsResult, MultipartUploadInfo,
    ObjectMetadata, ObjectStorage, ObjectSummary, PartInfo, PutObjectResult, StorageError,
    StoredObject,
};
