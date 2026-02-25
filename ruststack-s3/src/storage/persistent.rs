//! Persistent storage backend using SQLite

use super::traits::*;
use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;
use md5::{Digest, Md5};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

/// Persistent storage backend using SQLite
pub struct PersistentStorage {
    conn: Arc<Mutex<Connection>>,
    data_dir: Arc<std::path::PathBuf>,
}

impl PersistentStorage {
    pub fn new(data_dir: &Path) -> Result<Self, rusqlite::Error> {
        std::fs::create_dir_all(data_dir).ok();
        let db_path = data_dir.join("s3.db");
        let conn = Connection::open(db_path)?;

        // Initialize schema
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS buckets (
                name TEXT PRIMARY KEY,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS objects (
                bucket TEXT NOT NULL,
                key TEXT NOT NULL,
                data BLOB NOT NULL,
                etag TEXT NOT NULL,
                size INTEGER NOT NULL,
                content_type TEXT,
                content_encoding TEXT,
                content_disposition TEXT,
                content_language TEXT,
                cache_control TEXT,
                user_metadata TEXT,
                storage_class TEXT,
                last_modified TEXT NOT NULL,
                version_id TEXT,
                PRIMARY KEY (bucket, key, version_id)
            ) WITHOUT ROWID;

            CREATE INDEX IF NOT EXISTS idx_objects_bucket_key ON objects(bucket, key);

            CREATE TABLE IF NOT EXISTS multipart_uploads (
                bucket TEXT NOT NULL,
                key TEXT NOT NULL,
                upload_id TEXT NOT NULL,
                metadata TEXT,
                created_at TEXT NOT NULL,
                PRIMARY KEY (bucket, upload_id)
            );

            CREATE TABLE IF NOT EXISTS upload_parts (
                bucket TEXT NOT NULL,
                upload_id TEXT NOT NULL,
                part_number INTEGER NOT NULL,
                data BLOB NOT NULL,
                etag TEXT NOT NULL,
                size INTEGER NOT NULL,
                PRIMARY KEY (bucket, upload_id, part_number)
            );

            CREATE TABLE IF NOT EXISTS bucket_cors (
                bucket TEXT PRIMARY KEY,
                cors_config TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS bucket_versioning (
                bucket TEXT PRIMARY KEY,
                status TEXT NOT NULL,
                mfa_delete TEXT
            );
            "#,
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            data_dir: Arc::new(data_dir.to_path_buf()),
        })
    }

    fn compute_etag(data: &[u8]) -> String {
        let mut hasher = Md5::new();
        hasher.update(data);
        format!("\"{}\"", hex::encode(hasher.finalize()))
    }

    fn serialize_metadata(metadata: &ObjectMetadata) -> String {
        serde_json::to_string(metadata).unwrap_or_default()
    }

    fn deserialize_metadata(s: &str) -> ObjectMetadata {
        serde_json::from_str(s).unwrap_or_default()
    }
}

#[async_trait]
impl ObjectStorage for PersistentStorage {
    async fn create_bucket(&self, bucket: &str) -> Result<(), StorageError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO buckets (name, created_at) VALUES (?1, ?2)",
            params![bucket, Utc::now().to_rfc3339()],
        )
        .map_err(|e| {
            if e == rusqlite::Error::QueryReturnedNoRows {
                StorageError::BucketAlreadyExists(bucket.to_string())
            } else {
                StorageError::Internal(e.to_string())
            }
        })?;

        // Create bucket directory
        let bucket_dir = self.data_dir.join(bucket);
        std::fs::create_dir_all(&bucket_dir).ok();

        Ok(())
    }

    async fn delete_bucket(&self, bucket: &str) -> Result<(), StorageError> {
        let conn = self.conn.lock();

        // Check if bucket has objects
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM objects WHERE bucket = ?1",
                params![bucket],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if count > 0 {
            return Err(StorageError::BucketNotEmpty(bucket.to_string()));
        }

        conn.execute("DELETE FROM buckets WHERE name = ?1", params![bucket])
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        // Delete bucket directory
        let bucket_dir = self.data_dir.join(bucket);
        std::fs::remove_dir_all(&bucket_dir).ok();

        Ok(())
    }

    async fn bucket_exists(&self, bucket: &str) -> bool {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT 1 FROM buckets WHERE name = ?1",
            params![bucket],
            |_| Ok(()),
        )
        .is_ok()
    }

    async fn list_buckets(&self) -> Result<Vec<String>, StorageError> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT name FROM buckets")
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let buckets = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| StorageError::Internal(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(buckets)
    }

    async fn get_object(
        &self,
        bucket: &str,
        key: &str,
        _version_id: Option<&str>,
    ) -> Result<StoredObject, StorageError> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT data, etag, size, content_type, content_encoding, content_disposition, content_language, cache_control, user_metadata, storage_class, last_modified, version_id
             FROM objects WHERE bucket = ?1 AND key = ?2",
            params![bucket, key],
            |row| {
                let data: Vec<u8> = row.get(0)?;
                let user_metadata: String = row.get(9)?;
                Ok(StoredObject {
                    data: Bytes::from(data),
                    etag: row.get(1)?,
                    size: row.get::<_, i64>(2)? as u64,
                    metadata: Self::deserialize_metadata(&user_metadata),
                    last_modified: row.get::<_, String>(10)?.parse().unwrap_or_else(|_| Utc::now()),
                    version_id: row.get(11)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StorageError::ObjectNotFound {
                bucket: bucket.to_string(),
                key: key.to_string(),
            },
            _ => StorageError::Internal(e.to_string()),
        })
    }

    async fn put_object(
        &self,
        bucket: &str,
        key: &str,
        data: Bytes,
        metadata: ObjectMetadata,
    ) -> Result<PutObjectResult, StorageError> {
        let etag = Self::compute_etag(&data);
        let now = Utc::now().to_rfc3339();
        let metadata_json = Self::serialize_metadata(&metadata);

        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO objects (bucket, key, data, etag, size, content_type, content_encoding, content_disposition, content_language, cache_control, user_metadata, storage_class, last_modified, version_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                bucket,
                key,
                data.as_ref(),
                etag,
                data.len() as i64,
                metadata.content_type,
                metadata.content_encoding,
                metadata.content_disposition,
                metadata.content_language,
                metadata.cache_control,
                metadata_json,
                metadata.storage_class.unwrap_or_else(|| "STANDARD".to_string()),
                now,
                None::<String>
            ],
        )
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        // Also save to filesystem for direct access
        let obj_path = self.data_dir.join(bucket).join(key);
        if let Some(parent) = obj_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(&obj_path, &data).ok();

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
        let conn = self.conn.lock();
        let deleted = conn
            .execute(
                "DELETE FROM objects WHERE bucket = ?1 AND key = ?2",
                params![bucket, key],
            )
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        // Also delete from filesystem
        let obj_path = self.data_dir.join(bucket).join(key);
        std::fs::remove_file(&obj_path).ok();

        Ok(DeleteResult {
            deleted: deleted > 0,
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
        let conn = self.conn.lock();
        let prefix = prefix.unwrap_or("");
        let max_keys = max_keys as usize;

        let mut stmt = conn
            .prepare(
                "SELECT key, etag, size, last_modified, storage_class FROM objects WHERE bucket = ?1 AND key LIKE ?2",
            )
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let pattern = format!("{}%", prefix);
        let rows = stmt
            .query_map(params![bucket, pattern], |row| {
                Ok(ObjectSummary {
                    key: row.get(0)?,
                    etag: row.get(1)?,
                    size: row.get::<_, i64>(2)? as u64,
                    last_modified: row
                        .get::<_, String>(3)?
                        .parse()
                        .unwrap_or_else(|_| Utc::now()),
                    storage_class: row.get(4)?,
                })
            })
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let mut objects = Vec::new();
        let mut common_prefixes = std::collections::HashSet::new();

        for row in rows.flatten() {
            let key = &row.key;
            let suffix = &key[prefix.len()..];

            if let Some(delim) = delimiter {
                if let Some(pos) = suffix.find(delim) {
                    let common_prefix = format!("{}{}", prefix, &suffix[..=pos + delim.len() - 1]);
                    common_prefixes.insert(common_prefix);
                    continue;
                }
            }

            if objects.len() >= max_keys {
                break;
            }

            objects.push(row);
        }

        objects.sort_by(|a, b| a.key.cmp(&b.key));

        Ok(ListObjectsResult {
            objects,
            common_prefixes: common_prefixes.into_iter().collect(),
            is_truncated: false,
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
        let src = self.get_object(src_bucket, src_key, None).await?;
        self.put_object(dest_bucket, dest_key, src.data, src.metadata)
            .await
    }

    async fn create_multipart_upload(
        &self,
        bucket: &str,
        key: &str,
        metadata: ObjectMetadata,
    ) -> Result<String, StorageError> {
        let upload_id = Uuid::new_v4().to_string();
        let metadata_json = Self::serialize_metadata(&metadata);

        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO multipart_uploads (bucket, key, upload_id, metadata, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![bucket, key, upload_id, metadata_json, Utc::now().to_rfc3339()],
        )
        .map_err(|e| StorageError::Internal(e.to_string()))?;

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
        let etag = Self::compute_etag(&data);

        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO upload_parts (bucket, upload_id, part_number, data, etag, size) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![bucket, upload_id, part_number, data.as_ref(), etag, data.len() as i64],
        )
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(PartInfo {
            part_number,
            etag,
            size: data.len() as u64,
        })
    }

    async fn complete_multipart_upload(
        &self,
        bucket: &str,
        key: &str,
        upload_id: &str,
        parts: Vec<CompletedPart>,
    ) -> Result<CompleteResult, StorageError> {
        let conn = self.conn.lock();

        // Get upload metadata
        let metadata_json: String = conn
            .query_row(
                "SELECT metadata FROM multipart_uploads WHERE bucket = ?1 AND upload_id = ?2",
                params![bucket, upload_id],
                |row| row.get(0),
            )
            .map_err(|_| StorageError::UploadNotFound(upload_id.to_string()))?;

        // Get all parts
        let mut stmt = conn
            .prepare(
                "SELECT part_number, data FROM upload_parts WHERE bucket = ?1 AND upload_id = ?2 ORDER BY part_number",
            )
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let mut part_data: HashMap<i32, Vec<u8>> = HashMap::new();
        let mut rows = stmt
            .query(params![bucket, upload_id])
            .map_err(|e| StorageError::Internal(e.to_string()))?;
        while let Some(row) = rows
            .next()
            .map_err(|e| StorageError::Internal(e.to_string()))?
        {
            let part_num: i32 = row
                .get(0)
                .map_err(|e| StorageError::Internal(e.to_string()))?;
            let data: Vec<u8> = row
                .get(1)
                .map_err(|e| StorageError::Internal(e.to_string()))?;
            part_data.insert(part_num, data);
        }

        // Assemble parts
        let mut combined = Vec::new();
        for completed in &parts {
            let data = part_data.get(&completed.part_number).ok_or_else(|| {
                StorageError::Internal(format!("Part {} not found", completed.part_number))
            })?;
            combined.extend_from_slice(data);
        }

        // Compute multipart ETag
        let etag = format!(
            "\"{}-{}\"",
            Uuid::new_v4().to_string().replace("-", "")[..32].to_string(),
            parts.len()
        );

        // Store the completed object
        let metadata = Self::deserialize_metadata(&metadata_json);
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT OR REPLACE INTO objects (bucket, key, data, etag, size, content_type, content_encoding, content_disposition, content_language, cache_control, user_metadata, storage_class, last_modified, version_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                bucket,
                key,
                combined.as_slice(),
                etag,
                combined.len() as i64,
                metadata.content_type,
                metadata.content_encoding,
                metadata.content_disposition,
                metadata.content_language,
                metadata.cache_control,
                metadata_json,
                metadata.storage_class.unwrap_or_else(|| "STANDARD".to_string()),
                now,
                None::<String>
            ],
        )
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        // Clean up multipart upload
        conn.execute(
            "DELETE FROM multipart_uploads WHERE bucket = ?1 AND upload_id = ?2",
            params![bucket, upload_id],
        )
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        conn.execute(
            "DELETE FROM upload_parts WHERE bucket = ?1 AND upload_id = ?2",
            params![bucket, upload_id],
        )
        .map_err(|e| StorageError::Internal(e.to_string()))?;

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
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM multipart_uploads WHERE bucket = ?1 AND upload_id = ?2",
            params![bucket, upload_id],
        )
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        conn.execute(
            "DELETE FROM upload_parts WHERE bucket = ?1 AND upload_id = ?2",
            params![bucket, upload_id],
        )
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn list_multipart_uploads(
        &self,
        bucket: &str,
    ) -> Result<Vec<MultipartUploadInfo>, StorageError> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT key, upload_id, created_at FROM multipart_uploads WHERE bucket = ?1")
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let uploads = stmt
            .query_map(params![bucket], |row| {
                Ok(MultipartUploadInfo {
                    key: row.get(0)?,
                    upload_id: row.get(1)?,
                    initiated: row
                        .get::<_, String>(2)?
                        .parse()
                        .unwrap_or_else(|_| Utc::now()),
                })
            })
            .map_err(|e| StorageError::Internal(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(uploads)
    }

    async fn list_parts(
        &self,
        bucket: &str,
        _key: &str,
        upload_id: &str,
    ) -> Result<Vec<PartInfo>, StorageError> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT part_number, etag, size FROM upload_parts WHERE bucket = ?1 AND upload_id = ?2 ORDER BY part_number",
            )
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let parts = stmt
            .query_map(params![bucket, upload_id], |row| {
                Ok(PartInfo {
                    part_number: row.get(0)?,
                    etag: row.get(1)?,
                    size: row.get::<_, i64>(2)? as u64,
                })
            })
            .map_err(|e| StorageError::Internal(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(parts)
    }
}
