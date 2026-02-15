//! S3 storage backends

mod ephemeral;
mod traits;

pub use ephemeral::EphemeralStorage;
pub use traits::{ObjectStorage, StorageError, StoredObject, ObjectMetadata};
