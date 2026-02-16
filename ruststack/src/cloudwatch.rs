//! CloudWatch Logs stub for Lambda execution logs
//!
//! Provides minimal CloudWatch Logs API support for retrieving Lambda execution logs.

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::Response,
};
use bytes::Bytes;
use chrono::Utc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

/// Shared state for CloudWatch Logs
pub struct CloudWatchLogsState {
    /// Log groups: group_name -> LogGroup
    pub log_groups: DashMap<String, LogGroup>,
}

impl CloudWatchLogsState {
    pub fn new() -> Self {
        Self {
            log_groups: DashMap::new(),
        }
    }

    /// Store a log event (called by Lambda after invocation)
    #[allow(dead_code)]
    pub fn put_log_event(&self, group_name: &str, stream_name: &str, message: String) {
        let timestamp = Utc::now().timestamp_millis();

        // Get or create log group
        let group = self
            .log_groups
            .entry(group_name.to_string())
            .or_insert_with(|| LogGroup {
                log_group_name: group_name.to_string(),
                creation_time: timestamp,
                streams: DashMap::new(),
            });

        // Get or create log stream
        let mut stream = group
            .streams
            .entry(stream_name.to_string())
            .or_insert_with(|| LogStream {
                log_stream_name: stream_name.to_string(),
                creation_time: timestamp,
                first_event_timestamp: Some(timestamp),
                last_event_timestamp: Some(timestamp),
                last_ingestion_time: Some(timestamp),
                events: Vec::new(),
            });

        stream.events.push(LogEvent {
            timestamp,
            message,
            ingestion_time: timestamp,
        });
        stream.last_event_timestamp = Some(timestamp);
        stream.last_ingestion_time = Some(timestamp);
    }
}

impl Default for CloudWatchLogsState {
    fn default() -> Self {
        Self::new()
    }
}

/// A log group containing multiple streams
pub struct LogGroup {
    pub log_group_name: String,
    pub creation_time: i64,
    pub streams: DashMap<String, LogStream>,
}

/// A log stream containing events
pub struct LogStream {
    pub log_stream_name: String,
    pub creation_time: i64,
    pub first_event_timestamp: Option<i64>,
    pub last_event_timestamp: Option<i64>,
    pub last_ingestion_time: Option<i64>,
    pub events: Vec<LogEvent>,
}

/// A single log event
#[derive(Clone)]
pub struct LogEvent {
    pub timestamp: i64,
    pub message: String,
    pub ingestion_time: i64,
}

// === Request/Response types ===

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct DescribeLogGroupsRequest {
    log_group_name_prefix: Option<String>,
    limit: Option<i32>,
    next_token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DescribeLogGroupsResponse {
    log_groups: Vec<LogGroupInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogGroupInfo {
    log_group_name: String,
    creation_time: i64,
    stored_bytes: i64,
    arn: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct DescribeLogStreamsRequest {
    log_group_name: String,
    log_stream_name_prefix: Option<String>,
    order_by: Option<String>,
    descending: Option<bool>,
    limit: Option<i32>,
    next_token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DescribeLogStreamsResponse {
    log_streams: Vec<LogStreamInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogStreamInfo {
    log_stream_name: String,
    creation_time: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_event_timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_event_timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_ingestion_time: Option<i64>,
    stored_bytes: i64,
    arn: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct GetLogEventsRequest {
    log_group_name: String,
    log_stream_name: String,
    start_time: Option<i64>,
    end_time: Option<i64>,
    start_from_head: Option<bool>,
    limit: Option<i32>,
    next_token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GetLogEventsResponse {
    events: Vec<OutputLogEvent>,
    next_forward_token: String,
    next_backward_token: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutputLogEvent {
    timestamp: i64,
    message: String,
    ingestion_time: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateLogGroupRequest {
    log_group_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateLogStreamRequest {
    log_group_name: String,
    log_stream_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct PutLogEventsRequest {
    log_group_name: String,
    log_stream_name: String,
    log_events: Vec<InputLogEvent>,
    sequence_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteLogGroupRequest {
    log_group_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InputLogEvent {
    timestamp: i64,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PutLogEventsResponse {
    next_sequence_token: String,
}

// === Handler ===

pub async fn handle_logs_request(
    State(state): State<Arc<CloudWatchLogsState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Get action from x-amz-target header (format: Logs_20140328.ActionName)
    let target = match headers.get("x-amz-target") {
        Some(t) => match t.to_str() {
            Ok(s) => s,
            Err(_) => return error_response("SerializationException", "Invalid target header"),
        },
        None => return error_response("MissingAction", "Missing x-amz-target header"),
    };

    let action = target.split('.').next_back().unwrap_or(target);
    debug!(action = %action, "CloudWatch Logs request");

    // Parse body
    let body_json: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return error_response("SerializationException", &format!("Invalid JSON: {}", e)),
    };

    match action {
        "DescribeLogGroups" => handle_describe_log_groups(&state, body_json),
        "DescribeLogStreams" => handle_describe_log_streams(&state, body_json),
        "GetLogEvents" => handle_get_log_events(&state, body_json),
        "CreateLogGroup" => handle_create_log_group(&state, body_json),
        "CreateLogStream" => handle_create_log_stream(&state, body_json),
        "PutLogEvents" => handle_put_log_events(&state, body_json),
        "DeleteLogGroup" => handle_delete_log_group(&state, body_json),
        _ => error_response(
            "UnknownOperationException",
            &format!("Unknown operation: {}", action),
        ),
    }
}

fn handle_describe_log_groups(state: &CloudWatchLogsState, body: serde_json::Value) -> Response {
    let req: DescribeLogGroupsRequest = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => return error_response("InvalidParameterException", &e.to_string()),
    };

    let mut groups: Vec<LogGroupInfo> = state
        .log_groups
        .iter()
        .filter(|g| {
            req.log_group_name_prefix
                .as_ref()
                .map(|prefix| g.log_group_name.starts_with(prefix))
                .unwrap_or(true)
        })
        .map(|g| LogGroupInfo {
            log_group_name: g.log_group_name.clone(),
            creation_time: g.creation_time,
            stored_bytes: 0,
            arn: format!(
                "arn:aws:logs:us-east-1:000000000000:log-group:{}",
                g.log_group_name
            ),
        })
        .collect();

    // Sort by name
    groups.sort_by(|a, b| a.log_group_name.cmp(&b.log_group_name));

    // Apply limit
    let limit = req.limit.unwrap_or(50) as usize;
    groups.truncate(limit);

    json_response(DescribeLogGroupsResponse {
        log_groups: groups,
        next_token: None,
    })
}

fn handle_describe_log_streams(state: &CloudWatchLogsState, body: serde_json::Value) -> Response {
    let req: DescribeLogStreamsRequest = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => return error_response("InvalidParameterException", &e.to_string()),
    };

    let group = match state.log_groups.get(&req.log_group_name) {
        Some(g) => g,
        None => {
            return error_response(
                "ResourceNotFoundException",
                &format!(
                    "The specified log group does not exist: {}",
                    req.log_group_name
                ),
            )
        }
    };

    let mut streams: Vec<LogStreamInfo> = group
        .streams
        .iter()
        .filter(|s| {
            req.log_stream_name_prefix
                .as_ref()
                .map(|prefix| s.log_stream_name.starts_with(prefix))
                .unwrap_or(true)
        })
        .map(|s| LogStreamInfo {
            log_stream_name: s.log_stream_name.clone(),
            creation_time: s.creation_time,
            first_event_timestamp: s.first_event_timestamp,
            last_event_timestamp: s.last_event_timestamp,
            last_ingestion_time: s.last_ingestion_time,
            stored_bytes: 0,
            arn: format!(
                "arn:aws:logs:us-east-1:000000000000:log-group:{}:log-stream:{}",
                req.log_group_name, s.log_stream_name
            ),
        })
        .collect();

    // Sort by name or last event time
    match req.order_by.as_deref() {
        Some("LastEventTime") => {
            streams.sort_by(|a, b| {
                b.last_event_timestamp
                    .unwrap_or(0)
                    .cmp(&a.last_event_timestamp.unwrap_or(0))
            });
        }
        _ => {
            streams.sort_by(|a, b| a.log_stream_name.cmp(&b.log_stream_name));
        }
    }

    if req.descending.unwrap_or(false) {
        streams.reverse();
    }

    // Apply limit
    let limit = req.limit.unwrap_or(50) as usize;
    streams.truncate(limit);

    json_response(DescribeLogStreamsResponse {
        log_streams: streams,
        next_token: None,
    })
}

fn handle_get_log_events(state: &CloudWatchLogsState, body: serde_json::Value) -> Response {
    let req: GetLogEventsRequest = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => return error_response("InvalidParameterException", &e.to_string()),
    };

    let group = match state.log_groups.get(&req.log_group_name) {
        Some(g) => g,
        None => {
            return error_response(
                "ResourceNotFoundException",
                &format!(
                    "The specified log group does not exist: {}",
                    req.log_group_name
                ),
            )
        }
    };

    let stream = match group.streams.get(&req.log_stream_name) {
        Some(s) => s,
        None => {
            return error_response(
                "ResourceNotFoundException",
                &format!(
                    "The specified log stream does not exist: {}",
                    req.log_stream_name
                ),
            )
        }
    };

    let mut events: Vec<OutputLogEvent> = stream
        .events
        .iter()
        .filter(|e| {
            let after_start = req.start_time.map(|t| e.timestamp >= t).unwrap_or(true);
            let before_end = req.end_time.map(|t| e.timestamp <= t).unwrap_or(true);
            after_start && before_end
        })
        .map(|e| OutputLogEvent {
            timestamp: e.timestamp,
            message: e.message.clone(),
            ingestion_time: e.ingestion_time,
        })
        .collect();

    // Sort by timestamp
    if req.start_from_head.unwrap_or(false) {
        events.sort_by_key(|e| e.timestamp);
    } else {
        events.sort_by_key(|e| std::cmp::Reverse(e.timestamp));
    }

    // Apply limit
    let limit = req.limit.unwrap_or(10000) as usize;
    events.truncate(limit);

    // Generate tokens (just use timestamp-based tokens)
    let forward_token = events
        .last()
        .map(|e| format!("f/{}", e.timestamp))
        .unwrap_or_else(|| "f/0".to_string());
    let backward_token = events
        .first()
        .map(|e| format!("b/{}", e.timestamp))
        .unwrap_or_else(|| "b/0".to_string());

    json_response(GetLogEventsResponse {
        events,
        next_forward_token: forward_token,
        next_backward_token: backward_token,
    })
}

fn handle_create_log_group(state: &CloudWatchLogsState, body: serde_json::Value) -> Response {
    let req: CreateLogGroupRequest = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => return error_response("InvalidParameterException", &e.to_string()),
    };

    if state.log_groups.contains_key(&req.log_group_name) {
        return error_response(
            "ResourceAlreadyExistsException",
            &format!(
                "The specified log group already exists: {}",
                req.log_group_name
            ),
        );
    }

    let timestamp = Utc::now().timestamp_millis();
    state.log_groups.insert(
        req.log_group_name.clone(),
        LogGroup {
            log_group_name: req.log_group_name,
            creation_time: timestamp,
            streams: DashMap::new(),
        },
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-amz-json-1.1")
        .body(Body::from("{}"))
        .unwrap()
}

fn handle_create_log_stream(state: &CloudWatchLogsState, body: serde_json::Value) -> Response {
    let req: CreateLogStreamRequest = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => return error_response("InvalidParameterException", &e.to_string()),
    };

    let group = match state.log_groups.get(&req.log_group_name) {
        Some(g) => g,
        None => {
            return error_response(
                "ResourceNotFoundException",
                &format!(
                    "The specified log group does not exist: {}",
                    req.log_group_name
                ),
            )
        }
    };

    if group.streams.contains_key(&req.log_stream_name) {
        return error_response(
            "ResourceAlreadyExistsException",
            &format!(
                "The specified log stream already exists: {}",
                req.log_stream_name
            ),
        );
    }

    let timestamp = Utc::now().timestamp_millis();
    group.streams.insert(
        req.log_stream_name.clone(),
        LogStream {
            log_stream_name: req.log_stream_name,
            creation_time: timestamp,
            first_event_timestamp: None,
            last_event_timestamp: None,
            last_ingestion_time: None,
            events: Vec::new(),
        },
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-amz-json-1.1")
        .body(Body::from("{}"))
        .unwrap()
}

fn handle_put_log_events(state: &CloudWatchLogsState, body: serde_json::Value) -> Response {
    let req: PutLogEventsRequest = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => return error_response("InvalidParameterException", &e.to_string()),
    };

    let group = match state.log_groups.get(&req.log_group_name) {
        Some(g) => g,
        None => {
            return error_response(
                "ResourceNotFoundException",
                &format!(
                    "The specified log group does not exist: {}",
                    req.log_group_name
                ),
            )
        }
    };

    let mut stream = match group.streams.get_mut(&req.log_stream_name) {
        Some(s) => s,
        None => {
            return error_response(
                "ResourceNotFoundException",
                &format!(
                    "The specified log stream does not exist: {}",
                    req.log_stream_name
                ),
            )
        }
    };

    let ingestion_time = Utc::now().timestamp_millis();

    for event in req.log_events {
        if stream.first_event_timestamp.is_none() {
            stream.first_event_timestamp = Some(event.timestamp);
        }
        stream.last_event_timestamp = Some(event.timestamp);
        stream.last_ingestion_time = Some(ingestion_time);
        stream.events.push(LogEvent {
            timestamp: event.timestamp,
            message: event.message,
            ingestion_time,
        });
    }

    // Generate a sequence token
    let next_token = format!("{:016x}", ingestion_time as u64);

    json_response(PutLogEventsResponse {
        next_sequence_token: next_token,
    })
}

fn handle_delete_log_group(state: &CloudWatchLogsState, body: serde_json::Value) -> Response {
    let req: DeleteLogGroupRequest = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => return error_response("InvalidParameterException", &e.to_string()),
    };

    match state.log_groups.remove(&req.log_group_name) {
        Some(_) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/x-amz-json-1.1")
            .body(Body::from("{}"))
            .unwrap(),
        None => error_response(
            "ResourceNotFoundException",
            &format!(
                "The specified log group does not exist: {}",
                req.log_group_name
            ),
        ),
    }
}

fn json_response<T: Serialize>(body: T) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-amz-json-1.1")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap()
}

fn error_response(error_type: &str, message: &str) -> Response {
    let body = serde_json::json!({
        "__type": error_type,
        "message": message
    });

    let status = match error_type {
        "ResourceNotFoundException" => StatusCode::BAD_REQUEST,
        "ResourceAlreadyExistsException" => StatusCode::BAD_REQUEST,
        "InvalidParameterException" => StatusCode::BAD_REQUEST,
        _ => StatusCode::BAD_REQUEST,
    };

    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/x-amz-json-1.1")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap()
}
