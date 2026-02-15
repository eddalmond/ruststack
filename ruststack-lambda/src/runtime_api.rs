//! Lambda Runtime API implementation
//!
//! Implements the Lambda Runtime API that function code uses to receive
//! invocations and send responses.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{debug, error};

use crate::invocation::{Invocation, InvocationResult};

/// State for the Runtime API server
pub struct RuntimeApiState {
    /// Channel to receive invocations
    invocation_rx: RwLock<mpsc::Receiver<Invocation>>,
    /// Currently active invocation
    current_invocation: RwLock<Option<ActiveInvocation>>,
}

struct ActiveInvocation {
    request_id: String,
    response_tx: Option<oneshot::Sender<InvocationResult>>,
    function_version: String,
}

impl RuntimeApiState {
    pub fn new(invocation_rx: mpsc::Receiver<Invocation>) -> Self {
        Self {
            invocation_rx: RwLock::new(invocation_rx),
            current_invocation: RwLock::new(None),
        }
    }
}

/// Create the Runtime API router
pub fn runtime_api_router(state: Arc<RuntimeApiState>) -> Router {
    Router::new()
        .route(
            "/2018-06-01/runtime/invocation/next",
            get(get_next_invocation),
        )
        .route(
            "/2018-06-01/runtime/invocation/:request_id/response",
            post(post_invocation_response),
        )
        .route(
            "/2018-06-01/runtime/invocation/:request_id/error",
            post(post_invocation_error),
        )
        .route("/2018-06-01/runtime/init/error", post(post_init_error))
        .with_state(state)
}

/// GET /runtime/invocation/next
///
/// Blocks until an invocation is available, then returns it.
async fn get_next_invocation(State(state): State<Arc<RuntimeApiState>>) -> impl IntoResponse {
    debug!("Runtime requesting next invocation");

    // Wait for next invocation
    let invocation = {
        let mut rx = state.invocation_rx.write().await;
        match rx.recv().await {
            Some(inv) => inv,
            None => {
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("Invocation channel closed"))
                    .unwrap();
            }
        }
    };

    // Store the active invocation
    {
        let mut current = state.current_invocation.write().await;
        *current = Some(ActiveInvocation {
            request_id: invocation.request_id.clone(),
            response_tx: invocation.response_tx,
            function_version: "$LATEST".to_string(),
        });
    }

    debug!(request_id = %invocation.request_id, "Delivering invocation to runtime");

    Response::builder()
        .status(StatusCode::OK)
        .header("Lambda-Runtime-Aws-Request-Id", &invocation.request_id)
        .header(
            "Lambda-Runtime-Invoked-Function-Arn",
            &invocation.function_arn,
        )
        .header(
            "Lambda-Runtime-Deadline-Ms",
            invocation.deadline_ms.to_string(),
        )
        .body(Body::from(invocation.payload))
        .unwrap()
}

/// POST /runtime/invocation/{requestId}/response
///
/// Called by the function to return a successful response.
async fn post_invocation_response(
    State(state): State<Arc<RuntimeApiState>>,
    Path(request_id): Path<String>,
    body: Bytes,
) -> impl IntoResponse {
    debug!(request_id = %request_id, "Runtime sending response");

    let mut current = state.current_invocation.write().await;

    if let Some(invocation) = current.take() {
        if invocation.request_id != request_id {
            error!(
                expected = %invocation.request_id,
                received = %request_id,
                "Request ID mismatch"
            );
            return StatusCode::BAD_REQUEST;
        }

        if let Some(tx) = invocation.response_tx {
            let result = InvocationResult::success(body, invocation.function_version);
            let _ = tx.send(result);
        }

        StatusCode::ACCEPTED
    } else {
        error!(request_id = %request_id, "No active invocation");
        StatusCode::BAD_REQUEST
    }
}

/// POST /runtime/invocation/{requestId}/error
///
/// Called by the function to report an error.
async fn post_invocation_error(
    State(state): State<Arc<RuntimeApiState>>,
    Path(request_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let error_type = headers
        .get("Lambda-Runtime-Function-Error-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("Runtime.UnhandledError");

    debug!(request_id = %request_id, error_type = %error_type, "Runtime sending error");

    let mut current = state.current_invocation.write().await;

    if let Some(invocation) = current.take() {
        if let Some(tx) = invocation.response_tx {
            let error_message = String::from_utf8_lossy(&body).to_string();
            let result =
                InvocationResult::unhandled_error(error_message, invocation.function_version);
            let _ = tx.send(result);
        }

        StatusCode::ACCEPTED
    } else {
        StatusCode::BAD_REQUEST
    }
}

/// POST /runtime/init/error
///
/// Called when the runtime fails to initialize.
async fn post_init_error(headers: HeaderMap, body: Bytes) -> impl IntoResponse {
    let error_type = headers
        .get("Lambda-Runtime-Function-Error-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("Runtime.InitError");

    error!(
        error_type = %error_type,
        body = %String::from_utf8_lossy(&body),
        "Runtime initialization error"
    );

    StatusCode::ACCEPTED
}
