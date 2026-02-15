//! HTTP router for RustStack services

use crate::server::ServiceRegistry;
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Method, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{any, get},
    Router,
};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::info;

/// Create the main application router
pub fn create_router(services: ServiceRegistry) -> Router {
    let state = Arc::new(services);

    Router::new()
        // Health check endpoint
        .route("/health", get(health_check))
        .route("/_localstack/health", get(health_check)) // LocalStack compatibility
        // S3 routes - catch all for now
        .route("/", any(handle_request))
        .route("/*path", any(handle_request))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, r#"{"status": "running", "services": ["s3", "dynamodb", "lambda"]}"#)
}

async fn handle_request(
    State(services): State<Arc<ServiceRegistry>>,
    method: Method,
    headers: HeaderMap,
    request: Request<Body>,
) -> impl IntoResponse {
    let path = request.uri().path().to_string();
    let service = detect_service(&headers, &path);

    info!(
        method = %method,
        path = %path,
        service = %service,
        "Handling request"
    );

    match service {
        "s3" => services.handle_s3(request).await,
        "dynamodb" => services.handle_dynamodb(request).await,
        "lambda" => services.handle_lambda(request).await,
        _ => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Service not found"))
            .unwrap(),
    }
}

/// Detect which AWS service a request is targeting
fn detect_service(headers: &HeaderMap, path: &str) -> &'static str {
    // Check for DynamoDB
    if let Some(target) = headers.get("x-amz-target") {
        if let Ok(target_str) = target.to_str() {
            if target_str.starts_with("DynamoDB") {
                return "dynamodb";
            }
        }
    }

    // Check path-based routing
    if path.starts_with("/2015-03-31/functions") || path.starts_with("/lambda") {
        return "lambda";
    }

    // Default to S3 (most common)
    "s3"
}
