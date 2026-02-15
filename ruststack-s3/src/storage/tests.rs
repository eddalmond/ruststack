//! Comprehensive tests for S3 storage backends

use super::*;
use bytes::Bytes;
use std::collections::HashMap;

/// Test helper to create storage
fn storage() -> EphemeralStorage {
    EphemeralStorage::new()
}

// =============================================================================
// BUCKET OPERATIONS
// =============================================================================

mod bucket_tests {
    use super::*;

    #[tokio::test]
    async fn test_create_bucket() {
        let s = storage();
        s.create_bucket("my-bucket").await.unwrap();
        assert!(s.bucket_exists("my-bucket").await);
    }

    #[tokio::test]
    async fn test_create_bucket_already_exists() {
        let s = storage();
        s.create_bucket("my-bucket").await.unwrap();
        
        let result = s.create_bucket("my-bucket").await;
        assert!(matches!(result, Err(StorageError::BucketAlreadyExists(_))));
    }

    #[tokio::test]
    async fn test_delete_bucket() {
        let s = storage();
        s.create_bucket("my-bucket").await.unwrap();
        s.delete_bucket("my-bucket").await.unwrap();
        assert!(!s.bucket_exists("my-bucket").await);
    }

    #[tokio::test]
    async fn test_delete_bucket_not_found() {
        let s = storage();
        let result = s.delete_bucket("nonexistent").await;
        assert!(matches!(result, Err(StorageError::BucketNotFound(_))));
    }

    #[tokio::test]
    async fn test_delete_bucket_not_empty() {
        let s = storage();
        s.create_bucket("my-bucket").await.unwrap();
        s.put_object("my-bucket", "key", Bytes::from("data"), ObjectMetadata::default()).await.unwrap();
        
        let result = s.delete_bucket("my-bucket").await;
        assert!(matches!(result, Err(StorageError::BucketNotEmpty(_))));
    }

    #[tokio::test]
    async fn test_head_bucket_exists() {
        let s = storage();
        s.create_bucket("my-bucket").await.unwrap();
        assert!(s.bucket_exists("my-bucket").await);
    }

    #[tokio::test]
    async fn test_head_bucket_not_exists() {
        let s = storage();
        assert!(!s.bucket_exists("nonexistent").await);
    }

    #[tokio::test]
    async fn test_list_buckets_empty() {
        let s = storage();
        let buckets = s.list_buckets().await.unwrap();
        assert!(buckets.is_empty());
    }

    #[tokio::test]
    async fn test_list_buckets_multiple() {
        let s = storage();
        s.create_bucket("alpha").await.unwrap();
        s.create_bucket("beta").await.unwrap();
        s.create_bucket("gamma").await.unwrap();
        
        let buckets = s.list_buckets().await.unwrap();
        assert_eq!(buckets.len(), 3);
        assert!(buckets.contains(&"alpha".to_string()));
        assert!(buckets.contains(&"beta".to_string()));
        assert!(buckets.contains(&"gamma".to_string()));
    }

    #[tokio::test]
    async fn test_bucket_names_with_hyphens() {
        let s = storage();
        s.create_bucket("my-test-bucket-123").await.unwrap();
        assert!(s.bucket_exists("my-test-bucket-123").await);
    }

    #[tokio::test]
    async fn test_bucket_names_with_dots() {
        let s = storage();
        s.create_bucket("my.bucket.name").await.unwrap();
        assert!(s.bucket_exists("my.bucket.name").await);
    }
}

// =============================================================================
// OBJECT OPERATIONS
// =============================================================================

mod object_tests {
    use super::*;

    #[tokio::test]
    async fn test_put_object_simple() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        let result = s.put_object("bucket", "key", Bytes::from("hello"), ObjectMetadata::default()).await.unwrap();
        assert!(!result.etag.is_empty());
        assert!(result.etag.starts_with('"') && result.etag.ends_with('"'));
    }

    #[tokio::test]
    async fn test_put_object_bucket_not_found() {
        let s = storage();
        let result = s.put_object("nonexistent", "key", Bytes::from("data"), ObjectMetadata::default()).await;
        assert!(matches!(result, Err(StorageError::BucketNotFound(_))));
    }

    #[tokio::test]
    async fn test_get_object_simple() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        s.put_object("bucket", "key", Bytes::from("hello world"), ObjectMetadata::default()).await.unwrap();
        
        let obj = s.get_object("bucket", "key", None).await.unwrap();
        assert_eq!(&obj.data[..], b"hello world");
        assert_eq!(obj.size, 11);
    }

    #[tokio::test]
    async fn test_get_object_not_found() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        let result = s.get_object("bucket", "nonexistent", None).await;
        assert!(matches!(result, Err(StorageError::ObjectNotFound { .. })));
    }

    #[tokio::test]
    async fn test_get_object_bucket_not_found() {
        let s = storage();
        let result = s.get_object("nonexistent", "key", None).await;
        assert!(matches!(result, Err(StorageError::BucketNotFound(_))));
    }

    #[tokio::test]
    async fn test_delete_object_exists() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        s.put_object("bucket", "key", Bytes::from("data"), ObjectMetadata::default()).await.unwrap();
        
        let result = s.delete_object("bucket", "key", None).await.unwrap();
        assert!(result.deleted);
        
        // Verify it's gone
        let get_result = s.get_object("bucket", "key", None).await;
        assert!(get_result.is_err());
    }

    #[tokio::test]
    async fn test_delete_object_not_exists() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        // S3 returns success even for non-existent keys
        let result = s.delete_object("bucket", "nonexistent", None).await.unwrap();
        assert!(!result.deleted);
    }

    #[tokio::test]
    async fn test_delete_object_bucket_not_found() {
        let s = storage();
        let result = s.delete_object("nonexistent", "key", None).await;
        assert!(matches!(result, Err(StorageError::BucketNotFound(_))));
    }

    #[tokio::test]
    async fn test_head_object_exists() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        s.put_object("bucket", "key", Bytes::from("hello"), ObjectMetadata::default()).await.unwrap();
        
        let obj = s.get_object("bucket", "key", None).await.unwrap();
        assert_eq!(obj.size, 5);
        assert!(!obj.etag.is_empty());
    }

    #[tokio::test]
    async fn test_overwrite_object() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        s.put_object("bucket", "key", Bytes::from("original"), ObjectMetadata::default()).await.unwrap();
        s.put_object("bucket", "key", Bytes::from("updated"), ObjectMetadata::default()).await.unwrap();
        
        let obj = s.get_object("bucket", "key", None).await.unwrap();
        assert_eq!(&obj.data[..], b"updated");
    }

    #[tokio::test]
    async fn test_etag_consistency() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        // Same content should produce same ETag
        let data = Bytes::from("test content");
        let result1 = s.put_object("bucket", "key1", data.clone(), ObjectMetadata::default()).await.unwrap();
        let result2 = s.put_object("bucket", "key2", data, ObjectMetadata::default()).await.unwrap();
        
        assert_eq!(result1.etag, result2.etag);
    }
}

// =============================================================================
// OBJECT EDGE CASES
// =============================================================================

mod object_edge_cases {
    use super::*;

    #[tokio::test]
    async fn test_empty_object() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        s.put_object("bucket", "empty", Bytes::new(), ObjectMetadata::default()).await.unwrap();
        
        let obj = s.get_object("bucket", "empty", None).await.unwrap();
        assert_eq!(obj.size, 0);
        assert!(obj.data.is_empty());
    }

    #[tokio::test]
    async fn test_large_key_name() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        // S3 allows keys up to 1024 bytes
        let long_key = "a".repeat(1024);
        s.put_object("bucket", &long_key, Bytes::from("data"), ObjectMetadata::default()).await.unwrap();
        
        let obj = s.get_object("bucket", &long_key, None).await.unwrap();
        assert_eq!(&obj.data[..], b"data");
    }

    #[tokio::test]
    async fn test_key_with_special_characters() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        let special_keys = vec![
            "path/to/object",
            "key with spaces",
            "key+with+plus",
            "key=with=equals",
            "key&with&ampersand",
            "key/with/slashes/nested/deeply",
            "unicode-ключ-键",
            "emoji-🎉-test",
        ];
        
        for key in special_keys {
            s.put_object("bucket", key, Bytes::from(key.as_bytes()), ObjectMetadata::default()).await.unwrap();
            let obj = s.get_object("bucket", key, None).await.unwrap();
            assert_eq!(&obj.data[..], key.as_bytes(), "Failed for key: {}", key);
        }
    }

    #[tokio::test]
    async fn test_key_starting_with_slash() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        s.put_object("bucket", "/leading/slash", Bytes::from("data"), ObjectMetadata::default()).await.unwrap();
        let obj = s.get_object("bucket", "/leading/slash", None).await.unwrap();
        assert_eq!(&obj.data[..], b"data");
    }

    #[tokio::test]
    async fn test_key_with_consecutive_slashes() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        s.put_object("bucket", "a//b///c", Bytes::from("data"), ObjectMetadata::default()).await.unwrap();
        let obj = s.get_object("bucket", "a//b///c", None).await.unwrap();
        assert_eq!(&obj.data[..], b"data");
    }
}

// =============================================================================
// METADATA TESTS
// =============================================================================

mod metadata_tests {
    use super::*;

    #[tokio::test]
    async fn test_content_type() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        let mut meta = ObjectMetadata::default();
        meta.content_type = Some("application/json".to_string());
        
        s.put_object("bucket", "key", Bytes::from("{}"), meta).await.unwrap();
        
        let obj = s.get_object("bucket", "key", None).await.unwrap();
        assert_eq!(obj.metadata.content_type, Some("application/json".to_string()));
    }

    #[tokio::test]
    async fn test_user_metadata() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        let mut meta = ObjectMetadata::default();
        meta.user_metadata.insert("custom-header".to_string(), "custom-value".to_string());
        meta.user_metadata.insert("another".to_string(), "value".to_string());
        
        s.put_object("bucket", "key", Bytes::from("data"), meta).await.unwrap();
        
        let obj = s.get_object("bucket", "key", None).await.unwrap();
        assert_eq!(obj.metadata.user_metadata.get("custom-header"), Some(&"custom-value".to_string()));
        assert_eq!(obj.metadata.user_metadata.get("another"), Some(&"value".to_string()));
    }

    #[tokio::test]
    async fn test_all_standard_metadata_headers() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        let mut meta = ObjectMetadata::default();
        meta.content_type = Some("text/plain".to_string());
        meta.content_encoding = Some("gzip".to_string());
        meta.content_disposition = Some("attachment; filename=\"test.txt\"".to_string());
        meta.content_language = Some("en-US".to_string());
        meta.cache_control = Some("max-age=3600".to_string());
        
        s.put_object("bucket", "key", Bytes::from("data"), meta).await.unwrap();
        
        let obj = s.get_object("bucket", "key", None).await.unwrap();
        assert_eq!(obj.metadata.content_type, Some("text/plain".to_string()));
        assert_eq!(obj.metadata.content_encoding, Some("gzip".to_string()));
        assert_eq!(obj.metadata.content_disposition, Some("attachment; filename=\"test.txt\"".to_string()));
        assert_eq!(obj.metadata.content_language, Some("en-US".to_string()));
        assert_eq!(obj.metadata.cache_control, Some("max-age=3600".to_string()));
    }
}

// =============================================================================
// LIST OBJECTS TESTS
// =============================================================================

mod list_objects_tests {
    use super::*;

    #[tokio::test]
    async fn test_list_empty_bucket() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        let result = s.list_objects("bucket", None, None, None, 1000).await.unwrap();
        assert!(result.objects.is_empty());
        assert!(result.common_prefixes.is_empty());
    }

    #[tokio::test]
    async fn test_list_bucket_not_found() {
        let s = storage();
        let result = s.list_objects("nonexistent", None, None, None, 1000).await;
        assert!(matches!(result, Err(StorageError::BucketNotFound(_))));
    }

    #[tokio::test]
    async fn test_list_all_objects() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        for i in 0..5 {
            s.put_object("bucket", &format!("key{}", i), Bytes::from("data"), ObjectMetadata::default()).await.unwrap();
        }
        
        let result = s.list_objects("bucket", None, None, None, 1000).await.unwrap();
        assert_eq!(result.objects.len(), 5);
    }

    #[tokio::test]
    async fn test_list_with_prefix() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        s.put_object("bucket", "photos/2021/jan.jpg", Bytes::from("d"), ObjectMetadata::default()).await.unwrap();
        s.put_object("bucket", "photos/2021/feb.jpg", Bytes::from("d"), ObjectMetadata::default()).await.unwrap();
        s.put_object("bucket", "photos/2022/jan.jpg", Bytes::from("d"), ObjectMetadata::default()).await.unwrap();
        s.put_object("bucket", "videos/movie.mp4", Bytes::from("d"), ObjectMetadata::default()).await.unwrap();
        
        let result = s.list_objects("bucket", Some("photos/2021/"), None, None, 1000).await.unwrap();
        assert_eq!(result.objects.len(), 2);
        
        let result = s.list_objects("bucket", Some("photos/"), None, None, 1000).await.unwrap();
        assert_eq!(result.objects.len(), 3);
        
        let result = s.list_objects("bucket", Some("videos/"), None, None, 1000).await.unwrap();
        assert_eq!(result.objects.len(), 1);
    }

    #[tokio::test]
    async fn test_list_with_delimiter() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        s.put_object("bucket", "photos/2021/jan.jpg", Bytes::from("d"), ObjectMetadata::default()).await.unwrap();
        s.put_object("bucket", "photos/2021/feb.jpg", Bytes::from("d"), ObjectMetadata::default()).await.unwrap();
        s.put_object("bucket", "photos/2022/jan.jpg", Bytes::from("d"), ObjectMetadata::default()).await.unwrap();
        s.put_object("bucket", "videos/movie.mp4", Bytes::from("d"), ObjectMetadata::default()).await.unwrap();
        s.put_object("bucket", "readme.txt", Bytes::from("d"), ObjectMetadata::default()).await.unwrap();
        
        // List at root level with delimiter
        let result = s.list_objects("bucket", None, Some("/"), None, 1000).await.unwrap();
        assert_eq!(result.objects.len(), 1); // readme.txt
        assert!(result.common_prefixes.contains(&"photos/".to_string()));
        assert!(result.common_prefixes.contains(&"videos/".to_string()));
    }

    #[tokio::test]
    async fn test_list_with_prefix_and_delimiter() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        s.put_object("bucket", "photos/2021/jan.jpg", Bytes::from("d"), ObjectMetadata::default()).await.unwrap();
        s.put_object("bucket", "photos/2021/feb.jpg", Bytes::from("d"), ObjectMetadata::default()).await.unwrap();
        s.put_object("bucket", "photos/2022/jan.jpg", Bytes::from("d"), ObjectMetadata::default()).await.unwrap();
        
        let result = s.list_objects("bucket", Some("photos/"), Some("/"), None, 1000).await.unwrap();
        assert_eq!(result.objects.len(), 0);
        assert!(result.common_prefixes.contains(&"photos/2021/".to_string()));
        assert!(result.common_prefixes.contains(&"photos/2022/".to_string()));
    }

    #[tokio::test]
    async fn test_list_max_keys() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        for i in 0..10 {
            s.put_object("bucket", &format!("key{:02}", i), Bytes::from("d"), ObjectMetadata::default()).await.unwrap();
        }
        
        let result = s.list_objects("bucket", None, None, None, 3).await.unwrap();
        assert_eq!(result.objects.len(), 3);
    }

    #[tokio::test]
    async fn test_list_alphabetical_order() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        s.put_object("bucket", "c", Bytes::from("d"), ObjectMetadata::default()).await.unwrap();
        s.put_object("bucket", "a", Bytes::from("d"), ObjectMetadata::default()).await.unwrap();
        s.put_object("bucket", "b", Bytes::from("d"), ObjectMetadata::default()).await.unwrap();
        
        let result = s.list_objects("bucket", None, None, None, 1000).await.unwrap();
        let keys: Vec<&str> = result.objects.iter().map(|o| o.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }
}

// =============================================================================
// COPY OBJECT TESTS
// =============================================================================

mod copy_object_tests {
    use super::*;

    #[tokio::test]
    async fn test_copy_same_bucket() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        s.put_object("bucket", "source", Bytes::from("data"), ObjectMetadata::default()).await.unwrap();
        s.copy_object("bucket", "source", "bucket", "dest").await.unwrap();
        
        let obj = s.get_object("bucket", "dest", None).await.unwrap();
        assert_eq!(&obj.data[..], b"data");
    }

    #[tokio::test]
    async fn test_copy_different_buckets() {
        let s = storage();
        s.create_bucket("source-bucket").await.unwrap();
        s.create_bucket("dest-bucket").await.unwrap();
        
        s.put_object("source-bucket", "key", Bytes::from("data"), ObjectMetadata::default()).await.unwrap();
        s.copy_object("source-bucket", "key", "dest-bucket", "key").await.unwrap();
        
        let obj = s.get_object("dest-bucket", "key", None).await.unwrap();
        assert_eq!(&obj.data[..], b"data");
    }

    #[tokio::test]
    async fn test_copy_preserves_metadata() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        let mut meta = ObjectMetadata::default();
        meta.content_type = Some("text/plain".to_string());
        meta.user_metadata.insert("custom".to_string(), "value".to_string());
        
        s.put_object("bucket", "source", Bytes::from("data"), meta).await.unwrap();
        s.copy_object("bucket", "source", "bucket", "dest").await.unwrap();
        
        let obj = s.get_object("bucket", "dest", None).await.unwrap();
        assert_eq!(obj.metadata.content_type, Some("text/plain".to_string()));
        assert_eq!(obj.metadata.user_metadata.get("custom"), Some(&"value".to_string()));
    }

    #[tokio::test]
    async fn test_copy_source_not_found() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        let result = s.copy_object("bucket", "nonexistent", "bucket", "dest").await;
        assert!(matches!(result, Err(StorageError::ObjectNotFound { .. })));
    }

    #[tokio::test]
    async fn test_copy_dest_bucket_not_found() {
        let s = storage();
        s.create_bucket("source").await.unwrap();
        s.put_object("source", "key", Bytes::from("data"), ObjectMetadata::default()).await.unwrap();
        
        let result = s.copy_object("source", "key", "nonexistent", "key").await;
        assert!(matches!(result, Err(StorageError::BucketNotFound(_))));
    }
}

// =============================================================================
// MULTIPART UPLOAD TESTS
// =============================================================================

mod multipart_tests {
    use super::*;

    #[tokio::test]
    async fn test_create_multipart_upload() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        let upload_id = s.create_multipart_upload("bucket", "key", ObjectMetadata::default()).await.unwrap();
        assert!(!upload_id.is_empty());
    }

    #[tokio::test]
    async fn test_create_multipart_bucket_not_found() {
        let s = storage();
        let result = s.create_multipart_upload("nonexistent", "key", ObjectMetadata::default()).await;
        assert!(matches!(result, Err(StorageError::BucketNotFound(_))));
    }

    #[tokio::test]
    async fn test_upload_single_part() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        let upload_id = s.create_multipart_upload("bucket", "key", ObjectMetadata::default()).await.unwrap();
        let part = s.upload_part("bucket", "key", &upload_id, 1, Bytes::from("part data")).await.unwrap();
        
        assert_eq!(part.part_number, 1);
        assert!(!part.etag.is_empty());
        assert_eq!(part.size, 9);
    }

    #[tokio::test]
    async fn test_upload_multiple_parts() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        let upload_id = s.create_multipart_upload("bucket", "key", ObjectMetadata::default()).await.unwrap();
        
        let part1 = s.upload_part("bucket", "key", &upload_id, 1, Bytes::from("part1")).await.unwrap();
        let part2 = s.upload_part("bucket", "key", &upload_id, 2, Bytes::from("part2")).await.unwrap();
        let part3 = s.upload_part("bucket", "key", &upload_id, 3, Bytes::from("part3")).await.unwrap();
        
        assert_eq!(part1.part_number, 1);
        assert_eq!(part2.part_number, 2);
        assert_eq!(part3.part_number, 3);
    }

    #[tokio::test]
    async fn test_complete_multipart_upload() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        let upload_id = s.create_multipart_upload("bucket", "key", ObjectMetadata::default()).await.unwrap();
        
        let part1 = s.upload_part("bucket", "key", &upload_id, 1, Bytes::from("part1")).await.unwrap();
        let part2 = s.upload_part("bucket", "key", &upload_id, 2, Bytes::from("part2")).await.unwrap();
        
        let result = s.complete_multipart_upload(
            "bucket",
            "key",
            &upload_id,
            vec![
                CompletedPart { part_number: 1, etag: part1.etag },
                CompletedPart { part_number: 2, etag: part2.etag },
            ],
        ).await.unwrap();
        
        // Verify multipart ETag format: "hash-N"
        assert!(result.etag.ends_with("-2\""));
        
        // Verify combined content
        let obj = s.get_object("bucket", "key", None).await.unwrap();
        assert_eq!(&obj.data[..], b"part1part2");
    }

    #[tokio::test]
    async fn test_multipart_out_of_order_upload() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        let upload_id = s.create_multipart_upload("bucket", "key", ObjectMetadata::default()).await.unwrap();
        
        // Upload parts out of order
        let part3 = s.upload_part("bucket", "key", &upload_id, 3, Bytes::from("C")).await.unwrap();
        let part1 = s.upload_part("bucket", "key", &upload_id, 1, Bytes::from("A")).await.unwrap();
        let part2 = s.upload_part("bucket", "key", &upload_id, 2, Bytes::from("B")).await.unwrap();
        
        // Complete in correct order
        s.complete_multipart_upload(
            "bucket",
            "key",
            &upload_id,
            vec![
                CompletedPart { part_number: 1, etag: part1.etag },
                CompletedPart { part_number: 2, etag: part2.etag },
                CompletedPart { part_number: 3, etag: part3.etag },
            ],
        ).await.unwrap();
        
        let obj = s.get_object("bucket", "key", None).await.unwrap();
        assert_eq!(&obj.data[..], b"ABC");
    }

    #[tokio::test]
    async fn test_abort_multipart_upload() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        let upload_id = s.create_multipart_upload("bucket", "key", ObjectMetadata::default()).await.unwrap();
        s.upload_part("bucket", "key", &upload_id, 1, Bytes::from("data")).await.unwrap();
        
        s.abort_multipart_upload("bucket", "key", &upload_id).await.unwrap();
        
        // Object should not exist
        let result = s.get_object("bucket", "key", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_upload_part_invalid_upload_id() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        let result = s.upload_part("bucket", "key", "invalid-id", 1, Bytes::from("data")).await;
        assert!(matches!(result, Err(StorageError::UploadNotFound(_))));
    }

    #[tokio::test]
    async fn test_complete_invalid_upload_id() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        let result = s.complete_multipart_upload(
            "bucket",
            "key",
            "invalid-id",
            vec![],
        ).await;
        assert!(matches!(result, Err(StorageError::UploadNotFound(_))));
    }

    #[tokio::test]
    async fn test_multipart_with_metadata() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        let mut meta = ObjectMetadata::default();
        meta.content_type = Some("video/mp4".to_string());
        meta.user_metadata.insert("title".to_string(), "My Video".to_string());
        
        let upload_id = s.create_multipart_upload("bucket", "key", meta).await.unwrap();
        let part = s.upload_part("bucket", "key", &upload_id, 1, Bytes::from("video data")).await.unwrap();
        
        s.complete_multipart_upload(
            "bucket",
            "key",
            &upload_id,
            vec![CompletedPart { part_number: 1, etag: part.etag }],
        ).await.unwrap();
        
        let obj = s.get_object("bucket", "key", None).await.unwrap();
        assert_eq!(obj.metadata.content_type, Some("video/mp4".to_string()));
        assert_eq!(obj.metadata.user_metadata.get("title"), Some(&"My Video".to_string()));
    }

    #[tokio::test]
    async fn test_multiple_concurrent_uploads() {
        let s = storage();
        s.create_bucket("bucket").await.unwrap();
        
        let upload1 = s.create_multipart_upload("bucket", "file1", ObjectMetadata::default()).await.unwrap();
        let upload2 = s.create_multipart_upload("bucket", "file2", ObjectMetadata::default()).await.unwrap();
        
        let part1a = s.upload_part("bucket", "file1", &upload1, 1, Bytes::from("1A")).await.unwrap();
        let part2a = s.upload_part("bucket", "file2", &upload2, 1, Bytes::from("2A")).await.unwrap();
        let part1b = s.upload_part("bucket", "file1", &upload1, 2, Bytes::from("1B")).await.unwrap();
        let part2b = s.upload_part("bucket", "file2", &upload2, 2, Bytes::from("2B")).await.unwrap();
        
        s.complete_multipart_upload("bucket", "file1", &upload1, vec![
            CompletedPart { part_number: 1, etag: part1a.etag },
            CompletedPart { part_number: 2, etag: part1b.etag },
        ]).await.unwrap();
        
        s.complete_multipart_upload("bucket", "file2", &upload2, vec![
            CompletedPart { part_number: 1, etag: part2a.etag },
            CompletedPart { part_number: 2, etag: part2b.etag },
        ]).await.unwrap();
        
        let obj1 = s.get_object("bucket", "file1", None).await.unwrap();
        let obj2 = s.get_object("bucket", "file2", None).await.unwrap();
        
        assert_eq!(&obj1.data[..], b"1A1B");
        assert_eq!(&obj2.data[..], b"2A2B");
    }
}
