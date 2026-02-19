//! Lambda HTTP API handlers
//!
//! Implements the AWS Lambda HTTP API endpoints.

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    Json,
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::docker::{DockerExecutorConfig, ExecutorMode};
use crate::function::{Function, FunctionCode, FunctionConfig, Runtime};
use crate::invocation::InvocationType;
use crate::service::{LambdaService, LambdaServiceError};

/// Shared state for Lambda handlers
pub struct LambdaState {
    pub service: LambdaService,
}

impl LambdaState {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            service: LambdaService::new(),
        }
    }

    pub fn new_with_config(
        executor_mode: ExecutorMode,
        docker_config: DockerExecutorConfig,
    ) -> Self {
        Self {
            service: LambdaService::with_mode_and_config(executor_mode, docker_config),
        }
    }
}

impl Default for LambdaState {
    fn default() -> Self {
        Self::new()
    }
}

/// Create function request
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CreateFunctionRequest {
    pub function_name: String,
    pub runtime: String,
    pub role: String,
    pub handler: String,
    pub code: CodeRequest,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout: i32,
    #[serde(default = "default_memory_size")]
    pub memory_size: i32,
    #[serde(default)]
    pub environment: Option<EnvironmentRequest>,
}

fn default_timeout() -> i32 {
    3
}

fn default_memory_size() -> i32 {
    128
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CodeRequest {
    #[serde(default)]
    pub zip_file: Option<String>, // base64 encoded
    #[serde(default)]
    pub s3_bucket: Option<String>,
    #[serde(default)]
    pub s3_key: Option<String>,
    #[serde(default)]
    pub s3_object_version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct EnvironmentRequest {
    #[serde(default)]
    pub variables: HashMap<String, String>,
}

/// Update function code request
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UpdateFunctionCodeRequest {
    #[serde(default)]
    pub zip_file: Option<String>,
    #[serde(default)]
    pub s3_bucket: Option<String>,
    #[serde(default)]
    pub s3_key: Option<String>,
    #[serde(default)]
    pub s3_object_version: Option<String>,
}

/// Update function configuration request
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UpdateFunctionConfigRequest {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub handler: Option<String>,
    #[serde(default)]
    pub memory_size: Option<i32>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub timeout: Option<i32>,
    #[serde(default)]
    pub environment: Option<EnvironmentRequest>,
}

/// Function response (for both create and get)
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct FunctionResponse {
    pub function_name: String,
    pub function_arn: String,
    pub runtime: String,
    pub role: String,
    pub handler: String,
    pub code_size: i64,
    pub description: String,
    pub timeout: i32,
    pub memory_size: i32,
    pub last_modified: String,
    pub code_sha256: String,
    pub version: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<EnvironmentResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct EnvironmentResponse {
    pub variables: HashMap<String, String>,
}

impl From<&Function> for FunctionResponse {
    fn from(f: &Function) -> Self {
        Self {
            function_name: f.config.function_name.clone(),
            function_arn: f.arn.clone(),
            runtime: f.config.runtime.as_str().to_string(),
            role: f.config.role.clone(),
            handler: f.config.handler.clone(),
            code_size: f.code_size,
            description: f.config.description.clone().unwrap_or_default(),
            timeout: f.config.timeout,
            memory_size: f.config.memory_size,
            last_modified: f.last_modified.to_rfc3339(),
            code_sha256: f.code_sha256.clone(),
            version: f.version.clone(),
            state: format!("{:?}", f.state),
            environment: if f.config.environment.is_empty() {
                None
            } else {
                Some(EnvironmentResponse {
                    variables: f.config.environment.clone(),
                })
            },
        }
    }
}

/// GetFunction response with code location
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct GetFunctionResponse {
    pub configuration: FunctionResponse,
    pub code: FunctionCodeResponse,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct FunctionCodeResponse {
    pub repository_type: String,
    pub location: String,
}

/// ListFunctions response
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ListFunctionsResponse {
    pub functions: Vec<FunctionResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_marker: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListFunctionsQuery {
    #[serde(rename = "MaxItems")]
    pub max_items: Option<i32>,
    #[serde(rename = "Marker")]
    pub marker: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct InvokeQuery {
    #[serde(rename = "Qualifier")]
    pub qualifier: Option<String>,
}

/// Lambda error response
#[derive(Debug, Serialize)]
pub struct LambdaErrorResponse {
    #[serde(rename = "Type")]
    pub error_type: String,
    #[serde(rename = "Message", skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub message_lower: Option<String>,
}

// Error response helpers
fn resource_not_found(function_name: &str) -> Response {
    let body = serde_json::json!({
        "Type": "User",
        "Message": format!("Function not found: arn:aws:lambda:us-east-1:000000000000:function:{}", function_name),
        "message": format!("Function not found: arn:aws:lambda:us-east-1:000000000000:function:{}", function_name)
    });
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-amzn-ErrorType", "ResourceNotFoundException")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap()
}

fn resource_conflict(function_name: &str) -> Response {
    let body = serde_json::json!({
        "Type": "User",
        "Message": format!("Function already exist: {}", function_name),
        "message": format!("Function already exist: {}", function_name)
    });
    Response::builder()
        .status(StatusCode::CONFLICT)
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-amzn-ErrorType", "ResourceConflictException")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap()
}

fn invalid_parameter(message: &str) -> Response {
    let body = serde_json::json!({
        "Type": "User",
        "Message": message,
        "message": message
    });
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-amzn-ErrorType", "InvalidParameterValueException")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap()
}

fn internal_error(message: &str) -> Response {
    let body = serde_json::json!({
        "Type": "Server",
        "Message": message,
        "message": message
    });
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-amzn-ErrorType", "ServiceException")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap()
}

// === Handler functions ===

/// POST /2015-03-31/functions
/// Create a new Lambda function
pub async fn create_function(
    State(state): State<Arc<LambdaState>>,
    Json(req): Json<CreateFunctionRequest>,
) -> Response {
    info!(function_name = %req.function_name, "CreateFunction");

    // Parse runtime
    let runtime = match Runtime::from_str(&req.runtime) {
        Some(r) => r,
        None => return invalid_parameter(&format!("Invalid runtime: {}", req.runtime)),
    };

    // Parse code
    let code = if let Some(zip_data) = &req.code.zip_file {
        match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, zip_data) {
            Ok(data) => FunctionCode::ZipFile(data),
            Err(e) => return invalid_parameter(&format!("Invalid base64 in ZipFile: {}", e)),
        }
    } else if let (Some(bucket), Some(key)) = (&req.code.s3_bucket, &req.code.s3_key) {
        FunctionCode::S3 {
            bucket: bucket.clone(),
            key: key.clone(),
            version: req.code.s3_object_version.clone(),
        }
    } else {
        return invalid_parameter("Code must specify either ZipFile or S3Bucket/S3Key");
    };

    let config = FunctionConfig {
        function_name: req.function_name.clone(),
        runtime,
        handler: req.handler,
        role: req.role,
        memory_size: req.memory_size,
        timeout: req.timeout,
        environment: req.environment.map(|e| e.variables).unwrap_or_default(),
        description: req.description,
    };

    match state.service.create_function(config, code).await {
        Ok(function) => {
            let response = FunctionResponse::from(function.as_ref());
            Response::builder()
                .status(StatusCode::CREATED)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&response).unwrap()))
                .unwrap()
        }
        Err(LambdaServiceError::FunctionExists(name)) => resource_conflict(&name),
        Err(e) => internal_error(&e.to_string()),
    }
}

/// GET /2015-03-31/functions/{functionName}
/// Get function configuration and code location
pub async fn get_function(
    State(state): State<Arc<LambdaState>>,
    Path(function_name): Path<String>,
) -> Response {
    debug!(function_name = %function_name, "GetFunction");

    match state.service.get_function(&function_name) {
        Some(function) => {
            let response = GetFunctionResponse {
                configuration: FunctionResponse::from(function.as_ref()),
                code: FunctionCodeResponse {
                    repository_type: "S3".to_string(),
                    location: format!(
                        "https://awslambda-us-east-1-tasks.s3.us-east-1.amazonaws.com/snapshots/000000000000/{}-{}",
                        function_name,
                        uuid::Uuid::new_v4()
                    ),
                },
            };
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&response).unwrap()))
                .unwrap()
        }
        None => resource_not_found(&function_name),
    }
}

/// GET /2015-03-31/functions/{functionName}/configuration
/// Get function configuration only
pub async fn get_function_configuration(
    State(state): State<Arc<LambdaState>>,
    Path(function_name): Path<String>,
) -> Response {
    debug!(function_name = %function_name, "GetFunctionConfiguration");

    match state.service.get_function(&function_name) {
        Some(function) => {
            let response = FunctionResponse::from(function.as_ref());
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&response).unwrap()))
                .unwrap()
        }
        None => resource_not_found(&function_name),
    }
}

/// DELETE /2015-03-31/functions/{functionName}
/// Delete a Lambda function
pub async fn delete_function(
    State(state): State<Arc<LambdaState>>,
    Path(function_name): Path<String>,
) -> Response {
    info!(function_name = %function_name, "DeleteFunction");

    match state.service.delete_function(&function_name) {
        Ok(()) => Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Body::empty())
            .unwrap(),
        Err(LambdaServiceError::FunctionNotFound(_)) => resource_not_found(&function_name),
        Err(e) => internal_error(&e.to_string()),
    }
}

/// GET /2015-03-31/functions
/// List all Lambda functions
pub async fn list_functions(
    State(state): State<Arc<LambdaState>>,
    Query(_query): Query<ListFunctionsQuery>,
) -> Response {
    debug!("ListFunctions");

    let functions = state.service.list_functions();
    let response = ListFunctionsResponse {
        functions: functions
            .iter()
            .map(|f| FunctionResponse::from(f.as_ref()))
            .collect(),
        next_marker: None, // TODO: implement pagination
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&response).unwrap()))
        .unwrap()
}

/// PUT /2015-03-31/functions/{functionName}/code
/// Update function code
pub async fn update_function_code(
    State(state): State<Arc<LambdaState>>,
    Path(function_name): Path<String>,
    Json(req): Json<UpdateFunctionCodeRequest>,
) -> Response {
    info!(function_name = %function_name, "UpdateFunctionCode");

    // Parse code
    let code = if let Some(zip_data) = &req.zip_file {
        match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, zip_data) {
            Ok(data) => FunctionCode::ZipFile(data),
            Err(e) => return invalid_parameter(&format!("Invalid base64 in ZipFile: {}", e)),
        }
    } else if let (Some(bucket), Some(key)) = (&req.s3_bucket, &req.s3_key) {
        FunctionCode::S3 {
            bucket: bucket.clone(),
            key: key.clone(),
            version: req.s3_object_version.clone(),
        }
    } else {
        return invalid_parameter("Must specify either ZipFile or S3Bucket/S3Key");
    };

    match state
        .service
        .update_function_code(&function_name, code)
        .await
    {
        Ok(function) => {
            let response = FunctionResponse::from(function.as_ref());
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&response).unwrap()))
                .unwrap()
        }
        Err(LambdaServiceError::FunctionNotFound(_)) => resource_not_found(&function_name),
        Err(e) => internal_error(&e.to_string()),
    }
}

/// PUT /2015-03-31/functions/{functionName}/configuration
/// Update function configuration
pub async fn update_function_configuration(
    State(state): State<Arc<LambdaState>>,
    Path(function_name): Path<String>,
    Json(req): Json<UpdateFunctionConfigRequest>,
) -> Response {
    info!(function_name = %function_name, "UpdateFunctionConfiguration");

    // Parse runtime if provided
    let runtime = if let Some(rt) = &req.runtime {
        match Runtime::from_str(rt) {
            Some(r) => Some(r),
            None => return invalid_parameter(&format!("Invalid runtime: {}", rt)),
        }
    } else {
        None
    };

    match state
        .service
        .update_function_configuration(
            &function_name,
            req.description,
            req.handler,
            req.memory_size,
            req.role,
            runtime,
            req.timeout,
            req.environment.map(|e| e.variables),
        )
        .await
    {
        Ok(function) => {
            let response = FunctionResponse::from(function.as_ref());
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&response).unwrap()))
                .unwrap()
        }
        Err(LambdaServiceError::FunctionNotFound(_)) => resource_not_found(&function_name),
        Err(e) => internal_error(&e.to_string()),
    }
}

/// POST /2015-03-31/functions/{functionName}/invocations
/// Invoke a Lambda function
pub async fn invoke_function(
    State(state): State<Arc<LambdaState>>,
    Path(function_name): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    info!(function_name = %function_name, payload_size = %body.len(), "Invoke");

    // Parse invocation type from header
    let invocation_type = headers
        .get("X-Amz-Invocation-Type")
        .and_then(|v| v.to_str().ok())
        .and_then(InvocationType::from_str)
        .unwrap_or(InvocationType::RequestResponse);

    // Log type (for returning logs in response)
    let _log_type = headers
        .get("X-Amz-Log-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("None");

    match state
        .service
        .invoke(&function_name, body, invocation_type)
        .await
    {
        Ok(result) => {
            let mut builder = Response::builder()
                .status(StatusCode::from_u16(result.status_code as u16).unwrap_or(StatusCode::OK))
                .header("X-Amz-Executed-Version", &result.executed_version);

            if let Some(error) = &result.function_error {
                builder = builder.header("X-Amz-Function-Error", error);
            }

            if let Some(logs) = &result.log_result {
                builder = builder.header("X-Amz-Log-Result", logs);
            }

            builder
                .body(Body::from(result.payload.unwrap_or_default()))
                .unwrap()
        }
        Err(LambdaServiceError::FunctionNotFound(_)) => resource_not_found(&function_name),
        Err(e) => {
            error!(error = %e, "Invocation failed");
            internal_error(&e.to_string())
        }
    }
}
