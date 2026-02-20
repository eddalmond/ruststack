//! Storage backend traits

use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use thiserror::Error;

/// Errors from storage operations
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Bucket not found: {0}")]
    BucketNotFound(String),

    #[error("Object not found: {bucket}/{key}")]
    ObjectNotFound { bucket: String, key: String },

    #[error("Bucket already exists: {0}")]
    BucketAlreadyExists(String),

    #[error("Bucket not empty: {0}")]
    BucketNotEmpty(String),

    #[error("Invalid bucket name: {0}")]
    InvalidBucketName(String),

    #[error("Upload not found: {0}")]
    UploadNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Object metadata
#[derive(Debug, Clone, Default)]
pub struct ObjectMetadata {
    pub content_type: Option<String>,
    pub content_encoding: Option<String>,
    pub content_disposition: Option<String>,
    pub content_language: Option<String>,
    pub cache_control: Option<String>,
    pub user_metadata: HashMap<String, String>,
    pub storage_class: Option<String>,
}

/// A stored object
#[derive(Debug)]
pub struct StoredObject {
    pub data: Bytes,
    pub etag: String,
    pub size: u64,
    pub last_modified: DateTime<Utc>,
    pub metadata: ObjectMetadata,
    pub version_id: Option<String>,
}

/// Result of a PUT operation
#[derive(Debug)]
pub struct PutObjectResult {
    pub etag: String,
    pub version_id: Option<String>,
}

/// Result of a DELETE operation
#[derive(Debug)]
pub struct DeleteResult {
    pub deleted: bool,
    pub version_id: Option<String>,
    pub delete_marker: bool,
}

/// Result of a list operation
#[derive(Debug)]
pub struct ListObjectsResult {
    pub objects: Vec<ObjectSummary>,
    pub common_prefixes: Vec<String>,
    pub is_truncated: bool,
    pub next_continuation_token: Option<String>,
}

/// Summary of an object in a listing
#[derive(Debug)]
pub struct ObjectSummary {
    pub key: String,
    pub etag: String,
    pub size: u64,
    pub last_modified: DateTime<Utc>,
    pub storage_class: String,
}

/// Information about a multipart upload part
#[derive(Debug, Clone)]
pub struct PartInfo {
    pub part_number: i32,
    pub etag: String,
    pub size: u64,
}

/// Completed part for multipart upload
#[derive(Debug)]
pub struct CompletedPart {
    pub part_number: i32,
    pub etag: String,
}

/// Result of completing a multipart upload
#[derive(Debug)]
pub struct CompleteResult {
    pub etag: String,
    pub version_id: Option<String>,
}

/// Information about a multipart upload (for listing)
#[derive(Debug, Clone)]
pub struct MultipartUploadInfo {
    pub key: String,
    pub upload_id: String,
    pub initiated: DateTime<Utc>,
}

/// Abstract storage backend trait
#[async_trait]
pub trait ObjectStorage: Send + Sync {
    /// Create a bucket
    async fn create_bucket(&self, bucket: &str) -> Result<(), StorageError>;

    /// Delete a bucket
    async fn delete_bucket(&self, bucket: &str) -> Result<(), StorageError>;

    /// Check if a bucket exists
    async fn bucket_exists(&self, bucket: &str) -> bool;

    /// List all buckets
    async fn list_buckets(&self) -> Result<Vec<String>, StorageError>;

    /// Get an object
    async fn get_object(
        &self,
        bucket: &str,
        key: &str,
        version_id: Option<&str>,
    ) -> Result<StoredObject, StorageError>;

    /// Put an object
    async fn put_object(
        &self,
        bucket: &str,
        key: &str,
        data: Bytes,
        metadata: ObjectMetadata,
    ) -> Result<PutObjectResult, StorageError>;

    /// Delete an object
    async fn delete_object(
        &self,
        bucket: &str,
        key: &str,
        version_id: Option<&str>,
    ) -> Result<DeleteResult, StorageError>;

    /// List objects in a bucket
    async fn list_objects(
        &self,
        bucket: &str,
        prefix: Option<&str>,
        delimiter: Option<&str>,
        continuation_token: Option<&str>,
        max_keys: i32,
    ) -> Result<ListObjectsResult, StorageError>;

    /// Copy an object
    async fn copy_object(
        &self,
        src_bucket: &str,
        src_key: &str,
        dest_bucket: &str,
        dest_key: &str,
    ) -> Result<PutObjectResult, StorageError>;

    /// Create a multipart upload
    async fn create_multipart_upload(
        &self,
        bucket: &str,
        key: &str,
        metadata: ObjectMetadata,
    ) -> Result<String, StorageError>;

    /// Upload a part
    async fn upload_part(
        &self,
        bucket: &str,
        key: &str,
        upload_id: &str,
        part_number: i32,
        data: Bytes,
    ) -> Result<PartInfo, StorageError>;

    /// Complete a multipart upload
    async fn complete_multipart_upload(
        &self,
        bucket: &str,
        key: &str,
        upload_id: &str,
        parts: Vec<CompletedPart>,
    ) -> Result<CompleteResult, StorageError>;

    /// Abort a multipart upload
    async fn abort_multipart_upload(
        &self,
        bucket: &str,
        key: &str,
        upload_id: &str,
    ) -> Result<(), StorageError>;

    /// List multipart uploads in a bucket
    async fn list_multipart_uploads(
        &self,
        bucket: &str,
    ) -> Result<Vec<MultipartUploadInfo>, StorageError>;

    /// List parts in a multipart upload
    async fn list_parts(
        &self,
        bucket: &str,
        key: &str,
        upload_id: &str,
    ) -> Result<Vec<PartInfo>, StorageError>;
}
