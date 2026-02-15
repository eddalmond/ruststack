//! Lambda invocation handling

use crate::function::{Function, FunctionState};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::oneshot;

#[derive(Debug, Error)]
pub enum InvocationError {
    #[error("Function not found: {0}")]
    FunctionNotFound(String),

    #[error("Function not active: {0:?}")]
    FunctionNotActive(FunctionState),

    #[error("Invocation timeout after {0:?}")]
    Timeout(Duration),

    #[error("Container error: {0}")]
    ContainerError(String),

    #[error("Runtime error: {0}")]
    RuntimeError(String),

    #[error("Invalid payload: {0}")]
    InvalidPayload(String),
}

/// Invocation type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvocationType {
    /// Synchronous invocation (wait for response)
    RequestResponse,
    /// Asynchronous invocation (fire and forget)
    Event,
    /// Validation only (don't actually invoke)
    DryRun,
}

impl InvocationType {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "RequestResponse" => Some(Self::RequestResponse),
            "Event" => Some(Self::Event),
            "DryRun" => Some(Self::DryRun),
            _ => None,
        }
    }
}

/// Pending invocation
pub struct Invocation {
    pub request_id: String,
    pub function_arn: String,
    pub payload: Bytes,
    pub deadline_ms: i64,
    pub invocation_type: InvocationType,
    pub response_tx: Option<oneshot::Sender<InvocationResult>>,
}

/// Invocation result
#[derive(Debug)]
pub struct InvocationResult {
    pub status_code: i32,
    pub payload: Option<Bytes>,
    pub function_error: Option<String>,
    pub log_result: Option<String>,
    pub executed_version: String,
}

impl InvocationResult {
    pub fn success(payload: Bytes, version: String) -> Self {
        Self {
            status_code: 200,
            payload: Some(payload),
            function_error: None,
            log_result: None,
            executed_version: version,
        }
    }

    pub fn error(error: String, version: String) -> Self {
        Self {
            status_code: 200, // Lambda returns 200 even for handled errors
            payload: Some(Bytes::from(format!(r#"{{"errorMessage":"{}"}}"#, error))),
            function_error: Some("Handled".to_string()),
            log_result: None,
            executed_version: version,
        }
    }

    pub fn unhandled_error(error: String, version: String) -> Self {
        Self {
            status_code: 200,
            payload: Some(Bytes::from(format!(
                r#"{{"errorMessage":"{}","errorType":"Runtime.UnhandledError"}}"#,
                error
            ))),
            function_error: Some("Unhandled".to_string()),
            log_result: None,
            executed_version: version,
        }
    }
}

/// Lambda error response format
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LambdaErrorResponse {
    pub error_message: String,
    pub error_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack_trace: Option<Vec<String>>,
}

/// Lambda context passed to the function
#[derive(Debug, Clone, Serialize)]
pub struct LambdaContext {
    pub aws_request_id: String,
    pub invoked_function_arn: String,
    pub function_name: String,
    pub function_version: String,
    pub memory_limit_in_mb: i32,
    pub log_group_name: String,
    pub log_stream_name: String,
    pub deadline_ms: i64,
}

impl LambdaContext {
    pub fn new(function: &Function, request_id: &str, deadline_ms: i64) -> Self {
        Self {
            aws_request_id: request_id.to_string(),
            invoked_function_arn: function.qualified_arn(),
            function_name: function.config.function_name.clone(),
            function_version: function.version.clone(),
            memory_limit_in_mb: function.config.memory_size,
            log_group_name: format!("/aws/lambda/{}", function.config.function_name),
            log_stream_name: format!("2024/01/01/[$LATEST]{}", &request_id[..8]),
            deadline_ms,
        }
    }

    /// Get remaining time in milliseconds
    pub fn get_remaining_time_in_millis(&self) -> i64 {
        let now = chrono::Utc::now().timestamp_millis();
        (self.deadline_ms - now).max(0)
    }
}
