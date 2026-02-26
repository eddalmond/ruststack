//! Firehose in-memory storage

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::sync::Arc;

/// Delivery stream status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeliveryStreamStatus {
    Creating,
    Active,
    Deleting,
}

impl DeliveryStreamStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Creating => "CREATING",
            Self::Active => "ACTIVE",
            Self::Deleting => "DELETING",
        }
    }
}

/// A Firehose delivery stream
#[derive(Debug, Clone)]
pub struct DeliveryStream {
    pub delivery_stream_name: String,
    pub delivery_stream_arn: String,
    pub delivery_stream_status: DeliveryStreamStatus,
    pub delivery_stream_type: String, // DirectPut or KinesisStreamAsSource
    pub create_timestamp: DateTime<Utc>,
    pub destination_type: String, // S3, Redshift, Elasticsearch, etc.
    pub s3_bucket_arn: Option<String>,
    pub s3_prefix: Option<String>,
    pub buffering_hints: BufferingHints,
}

/// Buffering configuration
#[derive(Debug, Clone)]
pub struct BufferingHints {
    pub size_in_mbs: i32,
    pub interval_in_seconds: i32,
}

impl Default for BufferingHints {
    fn default() -> Self {
        Self {
            size_in_mbs: 5,
            interval_in_seconds: 300,
        }
    }
}

/// A buffered record
#[derive(Debug, Clone)]
pub struct BufferedRecord {
    pub data: Vec<u8>,
    pub received_at: DateTime<Utc>,
}

/// In-memory Firehose storage
#[derive(Debug, Default)]
pub struct FirehoseStorage {
    streams: DashMap<String, DeliveryStream>,
    /// Buffered records per stream (in real AWS, these would be flushed to S3)
    buffers: DashMap<String, Vec<BufferedRecord>>,
}

impl FirehoseStorage {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a delivery stream
    pub fn create_delivery_stream(
        &self,
        delivery_stream_name: &str,
        delivery_stream_type: &str,
        s3_bucket_arn: Option<String>,
        s3_prefix: Option<String>,
        buffering_hints: Option<BufferingHints>,
    ) -> Result<DeliveryStream, FirehoseError> {
        if self.streams.contains_key(delivery_stream_name) {
            return Err(FirehoseError::ResourceInUse(
                delivery_stream_name.to_string(),
            ));
        }

        let stream = DeliveryStream {
            delivery_stream_name: delivery_stream_name.to_string(),
            delivery_stream_arn: format!(
                "arn:aws:firehose:us-east-1:000000000000:deliverystream/{}",
                delivery_stream_name
            ),
            delivery_stream_status: DeliveryStreamStatus::Active,
            delivery_stream_type: delivery_stream_type.to_string(),
            create_timestamp: Utc::now(),
            destination_type: "ExtendedS3".to_string(),
            s3_bucket_arn,
            s3_prefix,
            buffering_hints: buffering_hints.unwrap_or_default(),
        };

        self.streams
            .insert(delivery_stream_name.to_string(), stream.clone());
        self.buffers
            .insert(delivery_stream_name.to_string(), Vec::new());

        Ok(stream)
    }

    /// Delete a delivery stream
    pub fn delete_delivery_stream(&self, delivery_stream_name: &str) -> Result<(), FirehoseError> {
        self.streams
            .remove(delivery_stream_name)
            .ok_or_else(|| FirehoseError::ResourceNotFound(delivery_stream_name.to_string()))?;
        self.buffers.remove(delivery_stream_name);
        Ok(())
    }

    /// Describe a delivery stream
    pub fn describe_delivery_stream(
        &self,
        delivery_stream_name: &str,
    ) -> Result<DeliveryStream, FirehoseError> {
        self.streams
            .get(delivery_stream_name)
            .map(|s| s.clone())
            .ok_or_else(|| FirehoseError::ResourceNotFound(delivery_stream_name.to_string()))
    }

    /// List delivery streams
    pub fn list_delivery_streams(&self, limit: Option<usize>) -> Vec<String> {
        let limit = limit.unwrap_or(100);
        self.streams
            .iter()
            .take(limit)
            .map(|s| s.key().clone())
            .collect()
    }

    /// Put a single record
    pub fn put_record(
        &self,
        delivery_stream_name: &str,
        data: Vec<u8>,
    ) -> Result<String, FirehoseError> {
        if !self.streams.contains_key(delivery_stream_name) {
            return Err(FirehoseError::ResourceNotFound(
                delivery_stream_name.to_string(),
            ));
        }

        let record = BufferedRecord {
            data,
            received_at: Utc::now(),
        };

        if let Some(mut buffer) = self.buffers.get_mut(delivery_stream_name) {
            buffer.push(record);
        }

        // Generate a record ID (in real AWS this would be meaningful)
        Ok(uuid::Uuid::new_v4().to_string())
    }

    /// Put multiple records
    pub fn put_record_batch(
        &self,
        delivery_stream_name: &str,
        records: Vec<Vec<u8>>,
    ) -> Result<PutRecordBatchResult, FirehoseError> {
        if !self.streams.contains_key(delivery_stream_name) {
            return Err(FirehoseError::ResourceNotFound(
                delivery_stream_name.to_string(),
            ));
        }

        let mut record_ids = Vec::new();
        let now = Utc::now();

        if let Some(mut buffer) = self.buffers.get_mut(delivery_stream_name) {
            for data in records {
                buffer.push(BufferedRecord {
                    data,
                    received_at: now,
                });
                record_ids.push(uuid::Uuid::new_v4().to_string());
            }
        }

        Ok(PutRecordBatchResult {
            failed_put_count: 0,
            record_ids,
        })
    }

    /// Get buffered records (for testing/debugging)
    pub fn get_buffered_records(&self, delivery_stream_name: &str) -> Vec<BufferedRecord> {
        self.buffers
            .get(delivery_stream_name)
            .map(|b| b.clone())
            .unwrap_or_default()
    }
}

/// Result of PutRecordBatch
#[derive(Debug)]
pub struct PutRecordBatchResult {
    pub failed_put_count: i32,
    pub record_ids: Vec<String>,
}

/// Firehose errors
#[derive(Debug, thiserror::Error)]
pub enum FirehoseError {
    #[error("Resource not found: {0}")]
    ResourceNotFound(String),

    #[error("Resource in use: {0}")]
    ResourceInUse(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
}

/// State for Firehose handlers
pub struct FirehoseState {
    pub storage: Arc<FirehoseStorage>,
}

impl FirehoseState {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(FirehoseStorage::new()),
        }
    }
}

impl Default for FirehoseState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_delivery_stream() {
        let storage = FirehoseStorage::new();
        let stream = storage
            .create_delivery_stream("test-stream", "DirectPut", None, None, None)
            .unwrap();

        assert_eq!(stream.delivery_stream_name, "test-stream");
        assert_eq!(stream.delivery_stream_status, DeliveryStreamStatus::Active);
    }

    #[test]
    fn test_put_record() {
        let storage = FirehoseStorage::new();
        storage
            .create_delivery_stream("test-stream", "DirectPut", None, None, None)
            .unwrap();

        let record_id = storage
            .put_record("test-stream", b"test data".to_vec())
            .unwrap();
        assert!(!record_id.is_empty());

        let records = storage.get_buffered_records("test-stream");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].data, b"test data".to_vec());
    }

    #[test]
    fn test_put_record_batch() {
        let storage = FirehoseStorage::new();
        storage
            .create_delivery_stream("test-stream", "DirectPut", None, None, None)
            .unwrap();

        let records = vec![
            b"record 1".to_vec(),
            b"record 2".to_vec(),
            b"record 3".to_vec(),
        ];

        let result = storage.put_record_batch("test-stream", records).unwrap();
        assert_eq!(result.failed_put_count, 0);
        assert_eq!(result.record_ids.len(), 3);

        let buffered = storage.get_buffered_records("test-stream");
        assert_eq!(buffered.len(), 3);
    }

    #[test]
    fn test_duplicate_stream_fails() {
        let storage = FirehoseStorage::new();
        storage
            .create_delivery_stream("test-stream", "DirectPut", None, None, None)
            .unwrap();

        let result = storage.create_delivery_stream("test-stream", "DirectPut", None, None, None);
        assert!(matches!(result, Err(FirehoseError::ResourceInUse(_))));
    }

    #[test]
    fn test_delete_delivery_stream() {
        let storage = FirehoseStorage::new();
        storage
            .create_delivery_stream("to-delete", "DirectPut", None, None, None)
            .unwrap();

        let result = storage.delete_delivery_stream("to-delete");
        assert!(result.is_ok());

        let result = storage.describe_delivery_stream("to-delete");
        assert!(matches!(result, Err(FirehoseError::ResourceNotFound(_))));
    }

    #[test]
    fn test_delete_nonexistent_stream_fails() {
        let storage = FirehoseStorage::new();

        let result = storage.delete_delivery_stream("nonexistent");
        assert!(matches!(result, Err(FirehoseError::ResourceNotFound(_))));
    }

    #[test]
    fn test_describe_delivery_stream() {
        let storage = FirehoseStorage::new();

        storage
            .create_delivery_stream("test-stream", "DirectPut", None, None, None)
            .unwrap();

        let stream = storage.describe_delivery_stream("test-stream").unwrap();

        assert_eq!(stream.delivery_stream_name, "test-stream");
        assert_eq!(stream.delivery_stream_type, "DirectPut");
    }

    #[test]
    fn test_describe_nonexistent_stream_fails() {
        let storage = FirehoseStorage::new();

        let result = storage.describe_delivery_stream("nonexistent");
        assert!(matches!(result, Err(FirehoseError::ResourceNotFound(_))));
    }

    #[test]
    fn test_list_delivery_streams() {
        let storage = FirehoseStorage::new();

        storage
            .create_delivery_stream("stream-1", "DirectPut", None, None, None)
            .unwrap();
        storage
            .create_delivery_stream("stream-2", "DirectPut", None, None, None)
            .unwrap();

        let streams = storage.list_delivery_streams(None);
        assert_eq!(streams.len(), 2);
    }

    #[test]
    fn test_list_delivery_streams_empty() {
        let storage = FirehoseStorage::new();
        let streams = storage.list_delivery_streams(None);
        assert!(streams.is_empty());
    }

    #[test]
    fn test_list_delivery_streams_with_limit() {
        let storage = FirehoseStorage::new();

        for i in 0..5 {
            storage
                .create_delivery_stream(&format!("stream-{}", i), "DirectPut", None, None, None)
                .unwrap();
        }

        let streams = storage.list_delivery_streams(Some(3));
        assert_eq!(streams.len(), 3);
    }

    #[test]
    fn test_delivery_stream_with_s3_destination() {
        let storage = FirehoseStorage::new();

        let stream = storage
            .create_delivery_stream(
                "s3-stream",
                "DirectPut",
                Some("arn:aws:s3:::my-bucket".to_string()),
                Some("firehose/".to_string()),
                None,
            )
            .unwrap();

        assert_eq!(
            stream.s3_bucket_arn,
            Some("arn:aws:s3:::my-bucket".to_string())
        );
        assert_eq!(stream.s3_prefix, Some("firehose/".to_string()));
        assert_eq!(stream.destination_type, "ExtendedS3");
    }

    #[test]
    fn test_delivery_stream_with_buffering_hints() {
        let storage = FirehoseStorage::new();

        let buffering = BufferingHints {
            size_in_mbs: 10,
            interval_in_seconds: 60,
        };

        let stream = storage
            .create_delivery_stream(
                "buffered-stream",
                "DirectPut",
                None,
                None,
                Some(buffering.clone()),
            )
            .unwrap();

        assert_eq!(stream.buffering_hints.size_in_mbs, 10);
        assert_eq!(stream.buffering_hints.interval_in_seconds, 60);
    }

    #[test]
    fn test_default_buffering_hints() {
        let storage = FirehoseStorage::new();

        let stream = storage
            .create_delivery_stream("default-buffered", "DirectPut", None, None, None)
            .unwrap();

        assert_eq!(stream.buffering_hints.size_in_mbs, 5);
        assert_eq!(stream.buffering_hints.interval_in_seconds, 300);
    }

    #[test]
    fn test_put_record_to_nonexistent_stream_fails() {
        let storage = FirehoseStorage::new();

        let result = storage.put_record("nonexistent", b"data".to_vec());
        assert!(matches!(result, Err(FirehoseError::ResourceNotFound(_))));
    }

    #[test]
    fn test_put_record_batch_to_nonexistent_stream_fails() {
        let storage = FirehoseStorage::new();

        let result = storage.put_record_batch("nonexistent", vec![b"data".to_vec()]);
        assert!(matches!(result, Err(FirehoseError::ResourceNotFound(_))));
    }
}
