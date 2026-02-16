//! HTTP handlers for Kinesis Firehose

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::Response,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};

use crate::storage::{BufferingHints, FirehoseError, FirehoseState};

/// Handle Firehose requests based on X-Amz-Target header
pub async fn handle_request(
    State(state): State<Arc<FirehoseState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let target = headers
        .get("x-amz-target")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    info!(target = %target, "Firehose request");

    match target {
        "Firehose_20150804.CreateDeliveryStream" => handle_create_delivery_stream(state, body).await,
        "Firehose_20150804.DeleteDeliveryStream" => handle_delete_delivery_stream(state, body).await,
        "Firehose_20150804.DescribeDeliveryStream" => handle_describe_delivery_stream(state, body).await,
        "Firehose_20150804.ListDeliveryStreams" => handle_list_delivery_streams(state, body).await,
        "Firehose_20150804.PutRecord" => handle_put_record(state, body).await,
        "Firehose_20150804.PutRecordBatch" => handle_put_record_batch(state, body).await,
        _ => {
            warn!(target = %target, "Unknown Firehose operation");
            error_response(
                StatusCode::BAD_REQUEST,
                "UnknownOperationException",
                &format!("Unknown operation: {}", target),
            )
        }
    }
}

// === Request/Response types ===

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CreateDeliveryStreamRequest {
    delivery_stream_name: String,
    delivery_stream_type: Option<String>,
    extended_s3_destination_configuration: Option<ExtendedS3Config>,
    s3_destination_configuration: Option<S3Config>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ExtendedS3Config {
    bucket_arn: Option<String>,
    #[serde(rename = "Prefix")]
    prefix: Option<String>,
    buffering_hints: Option<BufferingHintsConfig>,
    role_arn: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct S3Config {
    bucket_arn: Option<String>,
    #[serde(rename = "Prefix")]
    prefix: Option<String>,
    role_arn: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct BufferingHintsConfig {
    size_in_m_bs: Option<i32>,
    interval_in_seconds: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct CreateDeliveryStreamResponse {
    delivery_stream_arn: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DeleteDeliveryStreamRequest {
    delivery_stream_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DescribeDeliveryStreamRequest {
    delivery_stream_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct DescribeDeliveryStreamResponse {
    delivery_stream_description: DeliveryStreamDescription,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct DeliveryStreamDescription {
    delivery_stream_name: String,
    delivery_stream_arn: String,
    delivery_stream_status: String,
    delivery_stream_type: String,
    create_timestamp: f64,
    destinations: Vec<DestinationDescription>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct DestinationDescription {
    destination_id: String,
    extended_s3_destination_description: Option<ExtendedS3DestinationDescription>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct ExtendedS3DestinationDescription {
    #[serde(skip_serializing_if = "Option::is_none")]
    bucket_arn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prefix: Option<String>,
    buffering_hints: BufferingHintsResponse,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct BufferingHintsResponse {
    size_in_m_bs: i32,
    interval_in_seconds: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ListDeliveryStreamsRequest {
    limit: Option<usize>,
    exclusive_start_delivery_stream_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct ListDeliveryStreamsResponse {
    delivery_stream_names: Vec<String>,
    has_more_delivery_streams: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PutRecordRequest {
    delivery_stream_name: String,
    record: RecordInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RecordInput {
    data: String, // Base64 encoded
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct PutRecordResponse {
    record_id: String,
    encrypted: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PutRecordBatchRequest {
    delivery_stream_name: String,
    records: Vec<RecordInput>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct PutRecordBatchResponse {
    failed_put_count: i32,
    encrypted: bool,
    request_responses: Vec<PutRecordBatchResponseEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct PutRecordBatchResponseEntry {
    record_id: String,
}

// === Handlers ===

async fn handle_create_delivery_stream(state: Arc<FirehoseState>, body: Bytes) -> Response {
    let req: CreateDeliveryStreamRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, "ValidationException", &e.to_string()),
    };

    let delivery_stream_type = req.delivery_stream_type.unwrap_or_else(|| "DirectPut".to_string());

    // Extract S3 config from either extended or regular config
    let (s3_bucket_arn, s3_prefix, buffering) = if let Some(ext) = req.extended_s3_destination_configuration {
        let hints = ext.buffering_hints.map(|h| BufferingHints {
            size_in_mbs: h.size_in_m_bs.unwrap_or(5),
            interval_in_seconds: h.interval_in_seconds.unwrap_or(300),
        });
        (ext.bucket_arn, ext.prefix, hints)
    } else if let Some(s3) = req.s3_destination_configuration {
        (s3.bucket_arn, s3.prefix, None)
    } else {
        (None, None, None)
    };

    match state.storage.create_delivery_stream(
        &req.delivery_stream_name,
        &delivery_stream_type,
        s3_bucket_arn,
        s3_prefix,
        buffering,
    ) {
        Ok(stream) => {
            let response = CreateDeliveryStreamResponse {
                delivery_stream_arn: stream.delivery_stream_arn,
            };
            json_response(StatusCode::OK, &response)
        }
        Err(FirehoseError::ResourceInUse(name)) => {
            error_response(
                StatusCode::BAD_REQUEST,
                "ResourceInUseException",
                &format!("Delivery stream {} already exists", name),
            )
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, "InternalFailure", &e.to_string()),
    }
}

async fn handle_delete_delivery_stream(state: Arc<FirehoseState>, body: Bytes) -> Response {
    let req: DeleteDeliveryStreamRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, "ValidationException", &e.to_string()),
    };

    match state.storage.delete_delivery_stream(&req.delivery_stream_name) {
        Ok(()) => json_response(StatusCode::OK, &serde_json::json!({})),
        Err(FirehoseError::ResourceNotFound(name)) => {
            error_response(
                StatusCode::BAD_REQUEST,
                "ResourceNotFoundException",
                &format!("Delivery stream {} not found", name),
            )
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, "InternalFailure", &e.to_string()),
    }
}

async fn handle_describe_delivery_stream(state: Arc<FirehoseState>, body: Bytes) -> Response {
    let req: DescribeDeliveryStreamRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, "ValidationException", &e.to_string()),
    };

    match state.storage.describe_delivery_stream(&req.delivery_stream_name) {
        Ok(stream) => {
            let response = DescribeDeliveryStreamResponse {
                delivery_stream_description: DeliveryStreamDescription {
                    delivery_stream_name: stream.delivery_stream_name,
                    delivery_stream_arn: stream.delivery_stream_arn,
                    delivery_stream_status: stream.delivery_stream_status.as_str().to_string(),
                    delivery_stream_type: stream.delivery_stream_type,
                    create_timestamp: stream.create_timestamp.timestamp() as f64,
                    destinations: vec![DestinationDescription {
                        destination_id: "destinationId-000000000001".to_string(),
                        extended_s3_destination_description: Some(ExtendedS3DestinationDescription {
                            bucket_arn: stream.s3_bucket_arn,
                            prefix: stream.s3_prefix,
                            buffering_hints: BufferingHintsResponse {
                                size_in_m_bs: stream.buffering_hints.size_in_mbs,
                                interval_in_seconds: stream.buffering_hints.interval_in_seconds,
                            },
                        }),
                    }],
                },
            };
            json_response(StatusCode::OK, &response)
        }
        Err(FirehoseError::ResourceNotFound(name)) => {
            error_response(
                StatusCode::BAD_REQUEST,
                "ResourceNotFoundException",
                &format!("Delivery stream {} not found", name),
            )
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, "InternalFailure", &e.to_string()),
    }
}

async fn handle_list_delivery_streams(state: Arc<FirehoseState>, body: Bytes) -> Response {
    let req: ListDeliveryStreamsRequest = serde_json::from_slice(&body).unwrap_or(ListDeliveryStreamsRequest {
        limit: None,
        exclusive_start_delivery_stream_name: None,
    });

    let streams = state.storage.list_delivery_streams(req.limit);
    let response = ListDeliveryStreamsResponse {
        delivery_stream_names: streams,
        has_more_delivery_streams: false,
    };
    json_response(StatusCode::OK, &response)
}

async fn handle_put_record(state: Arc<FirehoseState>, body: Bytes) -> Response {
    let req: PutRecordRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, "ValidationException", &e.to_string()),
    };

    // Decode base64 data
    let data = match BASE64.decode(&req.record.data) {
        Ok(d) => d,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, "ValidationException", &format!("Invalid base64: {}", e)),
    };

    match state.storage.put_record(&req.delivery_stream_name, data) {
        Ok(record_id) => {
            let response = PutRecordResponse {
                record_id,
                encrypted: false,
            };
            json_response(StatusCode::OK, &response)
        }
        Err(FirehoseError::ResourceNotFound(name)) => {
            error_response(
                StatusCode::BAD_REQUEST,
                "ResourceNotFoundException",
                &format!("Delivery stream {} not found", name),
            )
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, "InternalFailure", &e.to_string()),
    }
}

async fn handle_put_record_batch(state: Arc<FirehoseState>, body: Bytes) -> Response {
    let req: PutRecordBatchRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, "ValidationException", &e.to_string()),
    };

    // Decode all records
    let mut records = Vec::new();
    for record in req.records {
        match BASE64.decode(&record.data) {
            Ok(d) => records.push(d),
            Err(e) => return error_response(StatusCode::BAD_REQUEST, "ValidationException", &format!("Invalid base64: {}", e)),
        }
    }

    match state.storage.put_record_batch(&req.delivery_stream_name, records) {
        Ok(result) => {
            let response = PutRecordBatchResponse {
                failed_put_count: result.failed_put_count,
                encrypted: false,
                request_responses: result
                    .record_ids
                    .into_iter()
                    .map(|id| PutRecordBatchResponseEntry { record_id: id })
                    .collect(),
            };
            json_response(StatusCode::OK, &response)
        }
        Err(FirehoseError::ResourceNotFound(name)) => {
            error_response(
                StatusCode::BAD_REQUEST,
                "ResourceNotFoundException",
                &format!("Delivery stream {} not found", name),
            )
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, "InternalFailure", &e.to_string()),
    }
}

// === Helpers ===

fn json_response<T: Serialize>(status: StatusCode, body: &T) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/x-amz-json-1.1")
        .body(Body::from(serde_json::to_string(body).unwrap()))
        .unwrap()
}

fn error_response(status: StatusCode, error_type: &str, message: &str) -> Response {
    let body = serde_json::json!({
        "__type": error_type,
        "message": message
    });
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/x-amz-json-1.1")
        .body(Body::from(body.to_string()))
        .unwrap()
}
