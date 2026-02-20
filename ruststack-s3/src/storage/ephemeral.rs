//! In-memory ephemeral storage backend

use super::traits::*;
use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;
use dashmap::DashMap;
use md5::{Digest, Md5};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// In-memory stored object
struct InMemoryObject {
    data: Bytes,
    etag: String,
    last_modified: chrono::DateTime<Utc>,
    metadata: ObjectMetadata,
}

/// In-memory bucket
#[allow(dead_code)]
struct InMemoryBucket {
    objects: DashMap<String, InMemoryObject>,
    multipart_uploads: DashMap<String, MultipartUpload>,
    created_at: chrono::DateTime<Utc>,
}

impl InMemoryBucket {
    fn new() -> Self {
        Self {
            objects: DashMap::new(),
            multipart_uploads: DashMap::new(),
            created_at: Utc::now(),
        }
    }
}

/// In-progress multipart upload
#[allow(dead_code)]
struct MultipartUpload {
    key: String,
    parts: HashMap<i32, InMemoryPart>,
    metadata: ObjectMetadata,
    created_at: chrono::DateTime<Utc>,
}

/// Uploaded part
#[allow(dead_code)]
struct InMemoryPart {
    data: Bytes,
    etag: String,
}

/// Ephemeral (in-memory) storage backend
pub struct EphemeralStorage {
    buckets: DashMap<String, Arc<InMemoryBucket>>,
}

impl Default for EphemeralStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl EphemeralStorage {
    pub fn new() -> Self {
        Self {
            buckets: DashMap::new(),
        }
    }

    fn compute_etag(data: &[u8]) -> String {
        let mut hasher = Md5::new();
        hasher.update(data);
        format!("\"{}\"", hex::encode(hasher.finalize()))
    }
}

#[async_trait]
impl ObjectStorage for EphemeralStorage {
    async fn create_bucket(&self, bucket: &str) -> Result<(), StorageError> {
        if self.buckets.contains_key(bucket) {
            return Err(StorageError::BucketAlreadyExists(bucket.to_string()));
        }
        self.buckets
            .insert(bucket.to_string(), Arc::new(InMemoryBucket::new()));
        Ok(())
    }

    async fn delete_bucket(&self, bucket: &str) -> Result<(), StorageError> {
        let bucket_ref = self
            .buckets
            .get(bucket)
            .ok_or_else(|| StorageError::BucketNotFound(bucket.to_string()))?;

        if !bucket_ref.objects.is_empty() {
            return Err(StorageError::BucketNotEmpty(bucket.to_string()));
        }

        drop(bucket_ref);
        self.buckets.remove(bucket);
        Ok(())
    }

    async fn bucket_exists(&self, bucket: &str) -> bool {
        self.buckets.contains_key(bucket)
    }

    async fn list_buckets(&self) -> Result<Vec<String>, StorageError> {
        Ok(self.buckets.iter().map(|r| r.key().clone()).collect())
    }

    async fn get_object(
        &self,
        bucket: &str,
        key: &str,
        _version_id: Option<&str>,
    ) -> Result<StoredObject, StorageError> {
        let bucket_ref = self
            .buckets
            .get(bucket)
            .ok_or_else(|| StorageError::BucketNotFound(bucket.to_string()))?;

        let obj = bucket_ref
            .objects
            .get(key)
            .ok_or_else(|| StorageError::ObjectNotFound {
                bucket: bucket.to_string(),
                key: key.to_string(),
            })?;

        Ok(StoredObject {
            data: obj.data.clone(),
            etag: obj.etag.clone(),
            size: obj.data.len() as u64,
            last_modified: obj.last_modified,
            metadata: obj.metadata.clone(),
            version_id: None,
        })
    }

    async fn put_object(
        &self,
        bucket: &str,
        key: &str,
        data: Bytes,
        metadata: ObjectMetadata,
    ) -> Result<PutObjectResult, StorageError> {
        let bucket_ref = self
            .buckets
            .get(bucket)
            .ok_or_else(|| StorageError::BucketNotFound(bucket.to_string()))?;

        let etag = Self::compute_etag(&data);

        bucket_ref.objects.insert(
            key.to_string(),
            InMemoryObject {
                data,
                etag: etag.clone(),
                last_modified: Utc::now(),
                metadata,
            },
        );

        Ok(PutObjectResult {
            etag,
            version_id: None,
        })
    }

    async fn delete_object(
        &self,
        bucket: &str,
        key: &str,
        _version_id: Option<&str>,
    ) -> Result<DeleteResult, StorageError> {
        let bucket_ref = self
            .buckets
            .get(bucket)
            .ok_or_else(|| StorageError::BucketNotFound(bucket.to_string()))?;

        let deleted = bucket_ref.objects.remove(key).is_some();

        Ok(DeleteResult {
            deleted,
            version_id: None,
            delete_marker: false,
        })
    }

    async fn list_objects(
        &self,
        bucket: &str,
        prefix: Option<&str>,
        delimiter: Option<&str>,
        _continuation_token: Option<&str>,
        max_keys: i32,
    ) -> Result<ListObjectsResult, StorageError> {
        let bucket_ref = self
            .buckets
            .get(bucket)
            .ok_or_else(|| StorageError::BucketNotFound(bucket.to_string()))?;

        let prefix = prefix.unwrap_or("");
        let max_keys = max_keys as usize;

        let mut objects = Vec::new();
        let mut common_prefixes = std::collections::HashSet::new();

        for entry in bucket_ref.objects.iter() {
            let key = entry.key();
            if !key.starts_with(prefix) {
                continue;
            }

            let suffix = &key[prefix.len()..];

            // Handle delimiter
            if let Some(delim) = delimiter {
                if let Some(pos) = suffix.find(delim) {
                    // This is a common prefix
                    let common_prefix = format!("{}{}", prefix, &suffix[..=pos + delim.len() - 1]);
                    common_prefixes.insert(common_prefix);
                    continue;
                }
            }

            if objects.len() >= max_keys {
                break;
            }

            objects.push(ObjectSummary {
                key: key.clone(),
                etag: entry.etag.clone(),
                size: entry.data.len() as u64,
                last_modified: entry.last_modified,
                storage_class: "STANDARD".to_string(),
            });
        }

        // Sort by key
        objects.sort_by(|a, b| a.key.cmp(&b.key));

        Ok(ListObjectsResult {
            objects,
            common_prefixes: common_prefixes.into_iter().collect(),
            is_truncated: false, // TODO: Implement pagination
            next_continuation_token: None,
        })
    }

    async fn copy_object(
        &self,
        src_bucket: &str,
        src_key: &str,
        dest_bucket: &str,
        dest_key: &str,
    ) -> Result<PutObjectResult, StorageError> {
        // Get source object
        let src = self.get_object(src_bucket, src_key, None).await?;

        // Put to destination
        self.put_object(dest_bucket, dest_key, src.data, src.metadata)
            .await
    }

    async fn create_multipart_upload(
        &self,
        bucket: &str,
        key: &str,
        metadata: ObjectMetadata,
    ) -> Result<String, StorageError> {
        let bucket_ref = self
            .buckets
            .get(bucket)
            .ok_or_else(|| StorageError::BucketNotFound(bucket.to_string()))?;

        let upload_id = Uuid::new_v4().to_string();

        bucket_ref.multipart_uploads.insert(
            upload_id.clone(),
            MultipartUpload {
                key: key.to_string(),
                parts: HashMap::new(),
                metadata,
                created_at: Utc::now(),
            },
        );

        Ok(upload_id)
    }

    async fn upload_part(
        &self,
        bucket: &str,
        _key: &str,
        upload_id: &str,
        part_number: i32,
        data: Bytes,
    ) -> Result<PartInfo, StorageError> {
        let bucket_ref = self
            .buckets
            .get(bucket)
            .ok_or_else(|| StorageError::BucketNotFound(bucket.to_string()))?;

        let mut upload = bucket_ref
            .multipart_uploads
            .get_mut(upload_id)
            .ok_or_else(|| StorageError::UploadNotFound(upload_id.to_string()))?;

        let etag = Self::compute_etag(&data);
        let size = data.len() as u64;

        upload.parts.insert(
            part_number,
            InMemoryPart {
                data,
                etag: etag.clone(),
            },
        );

        Ok(PartInfo {
            part_number,
            etag,
            size,
        })
    }

    async fn complete_multipart_upload(
        &self,
        bucket: &str,
        key: &str,
        upload_id: &str,
        parts: Vec<CompletedPart>,
    ) -> Result<CompleteResult, StorageError> {
        let bucket_ref = self
            .buckets
            .get(bucket)
            .ok_or_else(|| StorageError::BucketNotFound(bucket.to_string()))?;

        let upload = bucket_ref
            .multipart_uploads
            .remove(upload_id)
            .ok_or_else(|| StorageError::UploadNotFound(upload_id.to_string()))?
            .1;

        // Assemble parts
        let mut combined = Vec::new();
        for completed in &parts {
            let part = upload.parts.get(&completed.part_number).ok_or_else(|| {
                StorageError::Internal(format!("Part {} not found", completed.part_number))
            })?;
            combined.extend_from_slice(&part.data);
        }

        // Compute multipart ETag: MD5(concat(MD5(part1), MD5(part2), ...))-N
        let mut etag_parts = Vec::new();
        for completed in &parts {
            let part = upload.parts.get(&completed.part_number).unwrap();
            let mut hasher = Md5::new();
            hasher.update(&part.data);
            etag_parts.extend_from_slice(&hasher.finalize());
        }
        let mut final_hasher = Md5::new();
        final_hasher.update(&etag_parts);
        let etag = format!(
            "\"{}-{}\"",
            hex::encode(final_hasher.finalize()),
            parts.len()
        );

        // Store the object
        bucket_ref.objects.insert(
            key.to_string(),
            InMemoryObject {
                data: Bytes::from(combined),
                etag: etag.clone(),
                last_modified: Utc::now(),
                metadata: upload.metadata,
            },
        );

        Ok(CompleteResult {
            etag,
            version_id: None,
        })
    }

    async fn abort_multipart_upload(
        &self,
        bucket: &str,
        _key: &str,
        upload_id: &str,
    ) -> Result<(), StorageError> {
        let bucket_ref = self
            .buckets
            .get(bucket)
            .ok_or_else(|| StorageError::BucketNotFound(bucket.to_string()))?;

        bucket_ref.multipart_uploads.remove(upload_id);
        Ok(())
    }

    async fn list_multipart_uploads(
        &self,
        bucket: &str,
    ) -> Result<Vec<MultipartUploadInfo>, StorageError> {
        let bucket_ref = self
            .buckets
            .get(bucket)
            .ok_or_else(|| StorageError::BucketNotFound(bucket.to_string()))?;

        let uploads: Vec<MultipartUploadInfo> = bucket_ref
            .multipart_uploads
            .iter()
            .map(|entry| MultipartUploadInfo {
                key: entry.value().key.clone(),
                upload_id: entry.key().clone(),
                initiated: entry.value().created_at,
            })
            .collect();

        Ok(uploads)
    }

    async fn list_parts(
        &self,
        bucket: &str,
        _key: &str,
        upload_id: &str,
    ) -> Result<Vec<PartInfo>, StorageError> {
        let bucket_ref = self
            .buckets
            .get(bucket)
            .ok_or_else(|| StorageError::BucketNotFound(bucket.to_string()))?;

        let upload = bucket_ref
            .multipart_uploads
            .get(upload_id)
            .ok_or_else(|| StorageError::UploadNotFound(upload_id.to_string()))?;

        let parts: Vec<PartInfo> = upload
            .parts
            .iter()
            .map(|(num, part)| PartInfo {
                part_number: *num,
                etag: part.etag.clone(),
                size: part.data.len() as u64,
            })
            .collect();

        Ok(parts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bucket_operations() {
        let storage = EphemeralStorage::new();

        // Create bucket
        storage.create_bucket("test-bucket").await.unwrap();
        assert!(storage.bucket_exists("test-bucket").await);

        // List buckets
        let buckets = storage.list_buckets().await.unwrap();
        assert_eq!(buckets.len(), 1);
        assert!(buckets.contains(&"test-bucket".to_string()));

        // Delete bucket
        storage.delete_bucket("test-bucket").await.unwrap();
        assert!(!storage.bucket_exists("test-bucket").await);
    }

    #[tokio::test]
    async fn test_object_operations() {
        let storage = EphemeralStorage::new();
        storage.create_bucket("test-bucket").await.unwrap();

        // Put object
        let result = storage
            .put_object(
                "test-bucket",
                "test-key",
                Bytes::from("hello world"),
                ObjectMetadata::default(),
            )
            .await
            .unwrap();
        assert!(!result.etag.is_empty());

        // Get object
        let obj = storage
            .get_object("test-bucket", "test-key", None)
            .await
            .unwrap();
        assert_eq!(&obj.data[..], b"hello world");

        // Delete object
        storage
            .delete_object("test-bucket", "test-key", None)
            .await
            .unwrap();

        // Verify deleted
        let err = storage.get_object("test-bucket", "test-key", None).await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn test_multipart_upload() {
        let storage = EphemeralStorage::new();
        storage.create_bucket("test-bucket").await.unwrap();

        // Create multipart upload
        let upload_id = storage
            .create_multipart_upload("test-bucket", "large-object", ObjectMetadata::default())
            .await
            .unwrap();

        // Upload parts
        let part1 = storage
            .upload_part(
                "test-bucket",
                "large-object",
                &upload_id,
                1,
                Bytes::from("part1"),
            )
            .await
            .unwrap();

        let part2 = storage
            .upload_part(
                "test-bucket",
                "large-object",
                &upload_id,
                2,
                Bytes::from("part2"),
            )
            .await
            .unwrap();

        // Complete upload
        let result = storage
            .complete_multipart_upload(
                "test-bucket",
                "large-object",
                &upload_id,
                vec![
                    CompletedPart {
                        part_number: 1,
                        etag: part1.etag,
                    },
                    CompletedPart {
                        part_number: 2,
                        etag: part2.etag,
                    },
                ],
            )
            .await
            .unwrap();

        // Verify ETag format (multipart)
        assert!(result.etag.ends_with("-2\""));

        // Verify object contents
        let obj = storage
            .get_object("test-bucket", "large-object", None)
            .await
            .unwrap();
        assert_eq!(&obj.data[..], b"part1part2");
    }
}
