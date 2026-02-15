//! HTTP router for RustStack services

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, Method, Request, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{any, get},
    Router,
};
use bytes::Bytes;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::info;

use ruststack_s3::{
    storage::{EphemeralStorage, ObjectStorage},
    handlers::{self, S3State, ListObjectsQuery},
};

/// Service state for the main router
pub struct AppState {
    s3: Arc<S3State>,
    s3_enabled: bool,
    dynamodb_enabled: bool,
    lambda_enabled: bool,
}

impl AppState {
    pub fn new(s3_enabled: bool, dynamodb_enabled: bool, lambda_enabled: bool) -> Self {
        let storage: Arc<dyn ObjectStorage> = Arc::new(EphemeralStorage::new());
        Self {
            s3: Arc::new(S3State { storage }),
            s3_enabled,
            dynamodb_enabled,
            lambda_enabled,
        }
    }
}

/// Create the main application router
pub fn create_router(state: AppState) -> Router {
    let shared_state = Arc::new(state);

    Router::new()
        // Health check endpoint
        .route("/health", get(health_check))
        .route("/_localstack/health", get(health_check)) // LocalStack compatibility
        // S3 routes
        .route("/", any(handle_root))
        .route("/{bucket}", any(handle_bucket))
        .route("/{bucket}/{*key}", any(handle_object))
        .layer(TraceLayer::new_for_http())
        .with_state(shared_state)
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, r#"{"status": "running", "services": ["s3", "dynamodb", "lambda"]}"#)
}

async fn handle_root(
    State(state): State<Arc<AppState>>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    // Check for DynamoDB request
    if let Some(target) = headers.get("x-amz-target") {
        if let Ok(target_str) = target.to_str() {
            if target_str.starts_with("DynamoDB") {
                return handle_dynamodb_stub().await;
            }
        }
    }

    // Default to S3 ListBuckets
    if !state.s3_enabled {
        return service_disabled("s3");
    }

    handlers::handle_root(State(state.s3.clone()), method).await
}

async fn handle_bucket(
    State(state): State<Arc<AppState>>,
    Path(bucket): Path<String>,
    method: Method,
    Query(query): Query<ListObjectsQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !state.s3_enabled {
        return service_disabled("s3");
    }

    info!(bucket = %bucket, method = %method, "S3 bucket request");
    handlers::handle_bucket(State(state.s3.clone()), Path(bucket), method, Query(query), headers, body).await
}

async fn handle_object(
    State(state): State<Arc<AppState>>,
    Path((bucket, key)): Path<(String, String)>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !state.s3_enabled {
        return service_disabled("s3");
    }

    info!(bucket = %bucket, key = %key, method = %method, "S3 object request");
    handlers::handle_object(State(state.s3.clone()), Path((bucket, key)), method, headers, body).await
}

async fn handle_dynamodb_stub() -> Response {
    Response::builder()
        .status(StatusCode::NOT_IMPLEMENTED)
        .header(header::CONTENT_TYPE, "application/x-amz-json-1.0")
        .body(Body::from(r#"{"__type":"com.amazonaws.dynamodb.v20120810#ServiceUnavailableException","message":"DynamoDB service is not yet implemented"}"#))
        .unwrap()
}

fn service_disabled(service: &str) -> Response {
    Response::builder()
        .status(StatusCode::SERVICE_UNAVAILABLE)
        .body(Body::from(format!("Service '{}' is disabled", service)))
        .unwrap()
}
