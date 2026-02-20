//! S3 HTTP request handlers

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, Method, StatusCode},
    response::Response,
};
use bytes::Bytes;
use serde::Deserialize;
use std::sync::Arc;

use crate::storage::{CompletedPart, ObjectMetadata, ObjectStorage, StorageError};
use crate::xml::{
    format_complete_multipart_upload, format_create_multipart_upload, format_error,
    format_list_buckets, format_list_objects,
};

/// Shared state for S3 handlers
pub struct S3State {
    pub storage: Arc<dyn ObjectStorage>,
}

/// Query parameters for ListObjects
#[derive(Debug, Deserialize, Default)]
pub struct ListObjectsQuery {
    pub prefix: Option<String>,
    pub delimiter: Option<String>,
    #[serde(rename = "continuation-token")]
    pub continuation_token: Option<String>,
    #[serde(rename = "max-keys")]
    pub max_keys: Option<i32>,
    #[serde(rename = "list-type")]
    pub list_type: Option<i32>,
}

/// Query parameters for multipart upload operations
#[derive(Debug, Deserialize, Default)]
pub struct MultipartQuery {
    pub uploads: Option<String>,
    #[serde(rename = "uploadId")]
    pub upload_id: Option<String>,
    #[serde(rename = "partNumber")]
    pub part_number: Option<i32>,
}

/// Handle bucket-level operations
pub async fn handle_bucket(
    State(state): State<Arc<S3State>>,
    Path(bucket): Path<String>,
    method: Method,
    Query(query): Query<ListObjectsQuery>,
    headers: HeaderMap,
    _body: Bytes,
) -> Response {
    match method {
        Method::PUT => create_bucket(state, &bucket, headers).await,
        Method::DELETE => delete_bucket(state, &bucket).await,
        Method::HEAD => head_bucket(state, &bucket).await,
        Method::GET => list_objects(state, &bucket, query).await,
        _ => method_not_allowed(),
    }
}

/// Handle object-level operations (including multipart)
pub async fn handle_object(
    State(state): State<Arc<S3State>>,
    Path((bucket, key)): Path<(String, String)>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Get query string from headers and parse manually
    let query_string = headers.get("x-amz-query-string")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    
    let multipart = parse_multipart_query(query_string);
    
    // Check for multipart upload operations
    if multipart.uploads.is_some() && method == Method::POST {
        return create_multipart_upload(state, &bucket, &key, headers).await;
    }
    
    if let Some(ref upload_id) = multipart.upload_id {
        if method == Method::POST {
            return complete_multipart_upload(state, &bucket, &key, upload_id, body).await;
        }
        if method == Method::DELETE {
            return abort_multipart_upload(state, &bucket, &key, upload_id).await;
        }
    }
    
    if multipart.part_number.is_some() && method == Method::PUT {
        if let (Some(ref upload_id), Some(part_number)) = (&multipart.upload_id, multipart.part_number) {
            return upload_part(state, &bucket, &key, upload_id, part_number, body).await;
        }
    }

    // Regular object operations
    match method {
        Method::PUT => put_object(state, &bucket, &key, headers, body).await,
        Method::GET => get_object(state, &bucket, &key, headers).await,
        Method::DELETE => delete_object(state, &bucket, &key).await,
        Method::HEAD => head_object(state, &bucket, &key).await,
        _ => method_not_allowed(),
    }
}

fn parse_multipart_query(query: &str) -> MultipartQuery {
    let mut result = MultipartQuery::default();
    for pair in query.split('&') {
        let pair = pair.trim();
        if pair.is_empty() { continue; }
        if let Some((key, value)) = pair.split_once('=') {
            match key {
                "uploads" => result.uploads = Some(value.to_string()),
                "uploadId" => result.upload_id = Some(value.to_string()),
                "partNumber" => result.part_number = value.parse().ok(),
                _ => {}
            }
        }
    }
    result
}

/// Handle root-level operations (ListBuckets)
pub async fn handle_root(State(state): State<Arc<S3State>>, method: Method) -> Response {
    match method {
        Method::GET => list_buckets(state).await,
        _ => method_not_allowed(),
    }
}

// === Bucket Operations ===

async fn create_bucket(state: Arc<S3State>, bucket: &str, _headers: HeaderMap) -> Response {
    match state.storage.create_bucket(bucket).await {
        Ok(()) => Response::builder()
            .status(StatusCode::OK)
            .header("Location", format!("/{}", bucket))
            .body(Body::empty())
            .unwrap(),
        Err(StorageError::BucketAlreadyExists(_)) => Response::builder()
            .status(StatusCode::CONFLICT)
            .header(header::CONTENT_TYPE, "application/xml")
            .body(Body::from(format_error("BucketAlreadyOwnedByYou", "Your previous request to create the named bucket succeeded and you already own it.", bucket)))
            .unwrap(),
        Err(e) => internal_error(&e.to_string()),
    }
}

async fn delete_bucket(state: Arc<S3State>, bucket: &str) -> Response {
    match state.storage.delete_bucket(bucket).await {
        Ok(()) => Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Body::empty())
            .unwrap(),
        Err(StorageError::BucketNotFound(_)) => not_found_error(
            "NoSuchBucket",
            "The specified bucket does not exist",
            bucket,
        ),
        Err(StorageError::BucketNotEmpty(_)) => Response::builder()
            .status(StatusCode::CONFLICT)
            .header(header::CONTENT_TYPE, "application/xml")
            .body(Body::from(format_error(
                "BucketNotEmpty",
                "The bucket you tried to delete is not empty",
                bucket,
            )))
            .unwrap(),
        Err(e) => internal_error(&e.to_string()),
    }
}

async fn head_bucket(state: Arc<S3State>, bucket: &str) -> Response {
    if state.storage.bucket_exists(bucket).await {
        Response::builder()
            .status(StatusCode::OK)
            .header("x-amz-bucket-region", "us-east-1")
            .body(Body::empty())
            .unwrap()
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap()
    }
}

async fn list_buckets(state: Arc<S3State>) -> Response {
    match state.storage.list_buckets().await {
        Ok(buckets) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/xml")
            .body(Body::from(format_list_buckets(&buckets)))
            .unwrap(),
        Err(e) => internal_error(&e.to_string()),
    }
}

async fn list_objects(state: Arc<S3State>, bucket: &str, query: ListObjectsQuery) -> Response {
    let max_keys = query.max_keys.unwrap_or(1000);

    match state
        .storage
        .list_objects(
            bucket,
            query.prefix.as_deref(),
            query.delimiter.as_deref(),
            query.continuation_token.as_deref(),
            max_keys,
        )
        .await
    {
        Ok(result) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/xml")
            .body(Body::from(format_list_objects(
                bucket,
                &query.prefix,
                &query.delimiter,
                &result,
            )))
            .unwrap(),
        Err(StorageError::BucketNotFound(_)) => not_found_error(
            "NoSuchBucket",
            "The specified bucket does not exist",
            bucket,
        ),
        Err(e) => internal_error(&e.to_string()),
    }
}

// === Object Operations ===

async fn put_object(
    state: Arc<S3State>,
    bucket: &str,
    key: &str,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let metadata = extract_metadata(&headers);

    match state.storage.put_object(bucket, key, body, metadata).await {
        Ok(result) => Response::builder()
            .status(StatusCode::OK)
            .header("ETag", &result.etag)
            .body(Body::empty())
            .unwrap(),
        Err(StorageError::BucketNotFound(_)) => not_found_error(
            "NoSuchBucket",
            "The specified bucket does not exist",
            bucket,
        ),
        Err(e) => internal_error(&e.to_string()),
    }
}

async fn get_object(state: Arc<S3State>, bucket: &str, key: &str, _headers: HeaderMap) -> Response {
    match state.storage.get_object(bucket, key, None).await {
        Ok(obj) => {
            let mut builder = Response::builder()
                .status(StatusCode::OK)
                .header("ETag", &obj.etag)
                .header("Content-Length", obj.size.to_string())
                .header(
                    "Last-Modified",
                    obj.last_modified
                        .format("%a, %d %b %Y %H:%M:%S GMT")
                        .to_string(),
                );

            if let Some(ct) = &obj.metadata.content_type {
                builder = builder.header(header::CONTENT_TYPE, ct);
            } else {
                builder = builder.header(header::CONTENT_TYPE, "application/octet-stream");
            }

            builder.body(Body::from(obj.data)).unwrap()
        }
        Err(StorageError::BucketNotFound(_)) => not_found_error(
            "NoSuchBucket",
            "The specified bucket does not exist",
            bucket,
        ),
        Err(StorageError::ObjectNotFound { .. }) => {
            not_found_error("NoSuchKey", "The specified key does not exist.", key)
        }
        Err(e) => internal_error(&e.to_string()),
    }
}

async fn delete_object(state: Arc<S3State>, bucket: &str, key: &str) -> Response {
    match state.storage.delete_object(bucket, key, None).await {
        Ok(_) => Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Body::empty())
            .unwrap(),
        Err(StorageError::BucketNotFound(_)) => not_found_error(
            "NoSuchBucket",
            "The specified bucket does not exist",
            bucket,
        ),
        Err(e) => internal_error(&e.to_string()),
    }
}

async fn head_object(state: Arc<S3State>, bucket: &str, key: &str) -> Response {
    match state.storage.get_object(bucket, key, None).await {
        Ok(obj) => {
            let mut builder = Response::builder()
                .status(StatusCode::OK)
                .header("ETag", &obj.etag)
                .header("Content-Length", obj.size.to_string())
                .header(
                    "Last-Modified",
                    obj.last_modified
                        .format("%a, %d %b %Y %H:%M:%S GMT")
                        .to_string(),
                );

            if let Some(ct) = &obj.metadata.content_type {
                builder = builder.header(header::CONTENT_TYPE, ct);
            }

            builder.body(Body::empty()).unwrap()
        }
        Err(StorageError::BucketNotFound(_)) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap(),
        Err(StorageError::ObjectNotFound { .. }) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap(),
        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::empty())
            .unwrap(),
    }
}


// === Multipart Upload Operations ===

async fn create_multipart_upload(
    state: Arc<S3State>,
    bucket: &str,
    key: &str,
    headers: HeaderMap,
) -> Response {
    let metadata = extract_metadata(&headers);
    match state.storage.create_multipart_upload(bucket, key, metadata).await {
        Ok(upload_id) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/xml")
            .body(Body::from(format_create_multipart_upload(bucket, key, &upload_id)))
            .unwrap(),
        Err(StorageError::BucketNotFound(_)) => not_found_error("NoSuchBucket", "The specified bucket does not exist", bucket),
        Err(e) => internal_error(&e.to_string()),
    }
}

async fn upload_part(
    state: Arc<S3State>,
    bucket: &str,
    key: &str,
    upload_id: &str,
    part_number: i32,
    body: Bytes,
) -> Response {
    match state.storage.upload_part(bucket, key, upload_id, part_number, body).await {
        Ok(info) => Response::builder()
            .status(StatusCode::OK)
            .header("ETag", &info.etag)
            .body(Body::empty())
            .unwrap(),
        Err(StorageError::BucketNotFound(_)) => not_found_error("NoSuchBucket", "The specified bucket does not exist", bucket),
        Err(StorageError::UploadNotFound(_)) => not_found_error("NoSuchUpload", "The specified multipart upload does not exist.", ""),
        Err(e) => internal_error(&e.to_string()),
    }
}

async fn complete_multipart_upload(
    state: Arc<S3State>,
    bucket: &str,
    key: &str,
    upload_id: &str,
    body: Bytes,
) -> Response {
    let parts = match parse_complete_body(&body) {
        Ok(p) => p,
        Err(e) => return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(header::CONTENT_TYPE, "application/xml")
            .body(Body::from(format_error("MalformedXML", &e, "")))
            .unwrap(),
    };
    match state.storage.complete_multipart_upload(bucket, key, upload_id, parts).await {
        Ok(result) => {
            let location = format!("{}/{}", bucket, key);
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/xml")
                .body(Body::from(format_complete_multipart_upload(bucket, key, &result.etag, &location)))
                .unwrap()
        }
        Err(StorageError::BucketNotFound(_)) => not_found_error("NoSuchBucket", "The specified bucket does not exist", bucket),
        Err(StorageError::UploadNotFound(_)) => not_found_error("NoSuchUpload", "The specified multipart upload does not exist.", ""),
        Err(e) => internal_error(&e.to_string()),
    }
}

async fn abort_multipart_upload(
    state: Arc<S3State>,
    bucket: &str,
    key: &str,
    upload_id: &str,
) -> Response {
    match state.storage.abort_multipart_upload(bucket, key, upload_id).await {
        Ok(()) => Response::builder().status(StatusCode::NO_CONTENT).body(Body::empty()).unwrap(),
        Err(StorageError::BucketNotFound(_)) => not_found_error("NoSuchBucket", "The specified bucket does not exist", bucket),
        Err(StorageError::UploadNotFound(_)) => not_found_error("NoSuchUpload", "The specified multipart upload does not exist.", ""),
        Err(e) => internal_error(&e.to_string()),
    }
}

fn parse_complete_body(body: &[u8]) -> Result<Vec<CompletedPart>, String> {
    let body_str = String::from_utf8_lossy(body);
    let mut parts = Vec::new();
    let mut part_number = None;
    let mut etag = None;
    let mut in_part = false;
    
    for line in body_str.lines() {
        let line = line.trim();
        if line.contains("<Part>") {
            in_part = true;
            part_number = None;
            etag = None;
        } else if line.contains("</Part>") && in_part {
            if let (Some(pn), Some(et)) = (part_number.take(), etag.take()) {
                parts.push(CompletedPart { part_number: pn, etag: et });
            }
            in_part = false;
        } else if in_part {
            if line.starts_with("<PartNumber>") {
                if let Some(end) = line.find("</PartNumber>") {
                    part_number = line[12..end].parse().ok();
                }
            } else if line.starts_with("<ETag>") {
                if let Some(end) = line.find("</ETag>") {
                    etag = Some(line[6..end].trim().to_string());
                }
            }
        }
    }
    parts.sort_by(|a, b| a.part_number.cmp(&b.part_number));
    if parts.is_empty() { return Err("No parts found".to_string()); }
    Ok(parts)
}

// === Helper Functions ===

fn extract_metadata(headers: &HeaderMap) -> ObjectMetadata {
    let mut metadata = ObjectMetadata::default();

    if let Some(ct) = headers.get(header::CONTENT_TYPE) {
        metadata.content_type = ct.to_str().ok().map(String::from);
    }
    if let Some(ce) = headers.get(header::CONTENT_ENCODING) {
        metadata.content_encoding = ce.to_str().ok().map(String::from);
    }
    if let Some(cd) = headers.get(header::CONTENT_DISPOSITION) {
        metadata.content_disposition = cd.to_str().ok().map(String::from);
    }
    if let Some(cl) = headers.get(header::CONTENT_LANGUAGE) {
        metadata.content_language = cl.to_str().ok().map(String::from);
    }
    if let Some(cc) = headers.get(header::CACHE_CONTROL) {
        metadata.cache_control = cc.to_str().ok().map(String::from);
    }

    // Extract x-amz-meta-* headers
    for (key, value) in headers.iter() {
        let key_str = key.as_str().to_lowercase();
        if key_str.starts_with("x-amz-meta-") {
            if let Ok(v) = value.to_str() {
                let meta_key = key_str.strip_prefix("x-amz-meta-").unwrap().to_string();
                metadata.user_metadata.insert(meta_key, v.to_string());
            }
        }
    }

    metadata
}

fn method_not_allowed() -> Response {
    Response::builder()
        .status(StatusCode::METHOD_NOT_ALLOWED)
        .header(header::CONTENT_TYPE, "application/xml")
        .body(Body::from(format_error(
            "MethodNotAllowed",
            "The specified method is not allowed against this resource.",
            "",
        )))
        .unwrap()
}

fn not_found_error(code: &str, message: &str, resource: &str) -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, "application/xml")
        .body(Body::from(format_error(code, message, resource)))
        .unwrap()
}

fn internal_error(message: &str) -> Response {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CONTENT_TYPE, "application/xml")
        .body(Body::from(format_error("InternalError", message, "")))
        .unwrap()
}
