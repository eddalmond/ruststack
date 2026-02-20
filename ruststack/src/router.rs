//! HTTP router for RustStack services

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, Method, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{any, delete, get, post, put},
    Json, Router,
};
use bytes::Bytes;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::info;

use ruststack_apigateway::{handlers as apigateway_handlers, ApiGatewayState};
use ruststack_dynamodb::{handlers as dynamodb_handlers, DynamoDBState, DynamoDBStorage};
use ruststack_firehose::{handlers as firehose_handlers, FirehoseState};
use ruststack_iam::{handlers as iam_handlers, IamState};
use ruststack_lambda::{
    handlers::{
        self as lambda_handlers, CreateFunctionRequest, ListFunctionsQuery,
        UpdateFunctionCodeRequest, UpdateFunctionConfigRequest,
    },
    LambdaState,
};
use ruststack_s3::{
    handlers::{self, ListObjectsQuery, S3State},
    storage::{EphemeralStorage, ObjectStorage},
};
use ruststack_secretsmanager::{handlers as secrets_handlers, SecretsManagerState};
use ruststack_sns::{handlers as sns_handlers, SnsState};
use ruststack_sqs::{handlers as sqs_handlers, SqsState};

use crate::cloudwatch::{self, CloudWatchLogsState};

/// Service state for the main router
pub struct AppState {
    s3: Arc<S3State>,
    dynamodb: Arc<DynamoDBState>,
    lambda: Arc<LambdaState>,
    cloudwatch_logs: Arc<CloudWatchLogsState>,
    secretsmanager: Arc<SecretsManagerState>,
    iam: Arc<IamState>,
    apigateway: Arc<ApiGatewayState>,
    firehose: Arc<FirehoseState>,
    sqs: Arc<SqsState>,
    sns: Arc<SnsState>,
    s3_enabled: bool,
    dynamodb_enabled: bool,
    lambda_enabled: bool,
}

impl AppState {
    #[allow(dead_code)]
    pub fn new(s3_enabled: bool, dynamodb_enabled: bool, lambda_enabled: bool) -> Self {
        Self::new_with_lambda_config(
            s3_enabled,
            dynamodb_enabled,
            lambda_enabled,
            ruststack_lambda::ExecutorMode::Subprocess,
            ruststack_lambda::DockerExecutorConfig::default(),
        )
    }

    pub fn new_with_lambda_config(
        s3_enabled: bool,
        dynamodb_enabled: bool,
        lambda_enabled: bool,
        lambda_executor: ruststack_lambda::ExecutorMode,
        docker_config: ruststack_lambda::DockerExecutorConfig,
    ) -> Self {
        let storage: Arc<dyn ObjectStorage> = Arc::new(EphemeralStorage::new());
        let cloudwatch_logs = Arc::new(CloudWatchLogsState::new());
        Self {
            s3: Arc::new(S3State {
                storage: storage.clone(),
            }),
            dynamodb: Arc::new(DynamoDBState {
                storage: Arc::new(DynamoDBStorage::new()),
            }),
            lambda: Arc::new(LambdaState::new_with_config_and_s3(
                lambda_executor,
                docker_config,
                storage,
            )),
            cloudwatch_logs,
            secretsmanager: Arc::new(SecretsManagerState::new()),
            iam: Arc::new(IamState::new()),
            apigateway: Arc::new(ApiGatewayState::new()),
            firehose: Arc::new(FirehoseState::new()),
            sqs: Arc::new(ruststack_sqs::SqsState::new()),
            sns: Arc::new(ruststack_sns::SnsState::new()),
            s3_enabled,
            dynamodb_enabled,
            lambda_enabled,
        }
    }
}

/// Middleware to add x-amzn-requestid header to all responses
async fn add_request_id(request: axum::http::Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    let request_id = uuid::Uuid::new_v4().to_string();
    response
        .headers_mut()
        .insert("x-amzn-requestid", request_id.parse().unwrap());
    response
}

/// Create the main application router
pub fn create_router(state: AppState) -> Router {
    let shared_state = Arc::new(state);

    // Lambda routes - must be registered before S3 catch-all routes
    let lambda_routes = Router::new()
        // List functions
        .route("/2015-03-31/functions", get(list_functions))
        // Create function
        .route("/2015-03-31/functions", post(create_function))
        // Get function
        .route("/2015-03-31/functions/:function_name", get(get_function))
        // Delete function
        .route(
            "/2015-03-31/functions/:function_name",
            delete(delete_function),
        )
        // Get function configuration
        .route(
            "/2015-03-31/functions/:function_name/configuration",
            get(get_function_configuration),
        )
        // Update function configuration
        .route(
            "/2015-03-31/functions/:function_name/configuration",
            put(update_function_configuration),
        )
        // Update function code
        .route(
            "/2015-03-31/functions/:function_name/code",
            put(update_function_code),
        )
        // Invoke function
        .route(
            "/2015-03-31/functions/:function_name/invocations",
            post(invoke_function),
        );

    // API Gateway V2 routes
    let apigateway_routes = Router::new()
        .route("/v2/apis", post(apigw_create_api))
        .route("/v2/apis", get(apigw_list_apis))
        .route("/v2/apis/:api_id", get(apigw_get_api))
        .route("/v2/apis/:api_id", delete(apigw_delete_api))
        .route("/v2/apis/:api_id/routes", post(apigw_create_route))
        .route("/v2/apis/:api_id/routes", get(apigw_list_routes))
        .route("/v2/apis/:api_id/routes/:route_id", get(apigw_get_route))
        .route(
            "/v2/apis/:api_id/routes/:route_id",
            delete(apigw_delete_route),
        )
        .route(
            "/v2/apis/:api_id/integrations",
            post(apigw_create_integration),
        )
        .route(
            "/v2/apis/:api_id/integrations",
            get(apigw_list_integrations),
        )
        .route(
            "/v2/apis/:api_id/integrations/:integration_id",
            get(apigw_get_integration),
        )
        .route(
            "/v2/apis/:api_id/integrations/:integration_id",
            delete(apigw_delete_integration),
        )
        .route("/v2/apis/:api_id/stages", post(apigw_create_stage))
        .route("/v2/apis/:api_id/stages", get(apigw_list_stages))
        .route("/v2/apis/:api_id/stages/:stage_name", get(apigw_get_stage))
        .route(
            "/v2/apis/:api_id/stages/:stage_name",
            delete(apigw_delete_stage),
        );

    Router::new()
        // Health check endpoint
        .route("/health", get(health_check))
        .route("/_localstack/health", get(health_check)) // LocalStack compatibility
        // Lambda routes (before S3 catch-all)
        .merge(lambda_routes)
        // API Gateway V2 routes
        .merge(apigateway_routes)
        // S3 routes (catch-all)
        .route("/", any(handle_root))
        .route("/:bucket", any(handle_bucket))
        .route("/:bucket/*key", any(handle_object))
        .layer(middleware::from_fn(add_request_id))
        .layer(TraceLayer::new_for_http())
        .with_state(shared_state)
}

async fn health_check() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"status": "running", "services": {"s3": "available", "dynamodb": "available", "lambda": "available", "logs": "available", "secretsmanager": "available", "iam": "available", "apigatewayv2": "available", "firehose": "available", "sqs": "available", "sns": "available"}}"#,
        ))
        .unwrap()
}

// === Lambda handlers ===

async fn list_functions(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListFunctionsQuery>,
) -> Response {
    if !state.lambda_enabled {
        return service_disabled("lambda");
    }
    lambda_handlers::list_functions(State(state.lambda.clone()), Query(query)).await
}

async fn create_function(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateFunctionRequest>,
) -> Response {
    if !state.lambda_enabled {
        return service_disabled("lambda");
    }
    lambda_handlers::create_function(State(state.lambda.clone()), Json(req)).await
}

async fn get_function(
    State(state): State<Arc<AppState>>,
    Path(function_name): Path<String>,
) -> Response {
    if !state.lambda_enabled {
        return service_disabled("lambda");
    }
    lambda_handlers::get_function(State(state.lambda.clone()), Path(function_name)).await
}

async fn delete_function(
    State(state): State<Arc<AppState>>,
    Path(function_name): Path<String>,
) -> Response {
    if !state.lambda_enabled {
        return service_disabled("lambda");
    }
    lambda_handlers::delete_function(State(state.lambda.clone()), Path(function_name)).await
}

async fn get_function_configuration(
    State(state): State<Arc<AppState>>,
    Path(function_name): Path<String>,
) -> Response {
    if !state.lambda_enabled {
        return service_disabled("lambda");
    }
    lambda_handlers::get_function_configuration(State(state.lambda.clone()), Path(function_name))
        .await
}

async fn update_function_configuration(
    State(state): State<Arc<AppState>>,
    Path(function_name): Path<String>,
    Json(req): Json<UpdateFunctionConfigRequest>,
) -> Response {
    if !state.lambda_enabled {
        return service_disabled("lambda");
    }
    lambda_handlers::update_function_configuration(
        State(state.lambda.clone()),
        Path(function_name),
        Json(req),
    )
    .await
}

async fn update_function_code(
    State(state): State<Arc<AppState>>,
    Path(function_name): Path<String>,
    Json(req): Json<UpdateFunctionCodeRequest>,
) -> Response {
    if !state.lambda_enabled {
        return service_disabled("lambda");
    }
    lambda_handlers::update_function_code(
        State(state.lambda.clone()),
        Path(function_name),
        Json(req),
    )
    .await
}

async fn invoke_function(
    State(state): State<Arc<AppState>>,
    Path(function_name): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !state.lambda_enabled {
        return service_disabled("lambda");
    }
    lambda_handlers::invoke_function(
        State(state.lambda.clone()),
        Path(function_name),
        headers,
        body,
    )
    .await
}

// === S3 / DynamoDB / CloudWatch Logs routing ===

async fn handle_root(
    State(state): State<Arc<AppState>>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Check for DynamoDB request
    if let Some(target) = headers.get("x-amz-target") {
        if let Ok(target_str) = target.to_str() {
            if target_str.starts_with("DynamoDB") {
                if !state.dynamodb_enabled {
                    return service_disabled("dynamodb");
                }
                return dynamodb_handlers::handle_request(
                    State(state.dynamodb.clone()),
                    headers,
                    body,
                )
                .await;
            }
            // CloudWatch Logs
            if target_str.starts_with("Logs_") {
                return cloudwatch::handle_logs_request(
                    State(state.cloudwatch_logs.clone()),
                    headers,
                    body,
                )
                .await;
            }
            // Secrets Manager
            if target_str.starts_with("secretsmanager") {
                return secrets_handlers::handle_request(
                    State(state.secretsmanager.clone()),
                    headers,
                    body,
                )
                .await;
            }
            // SQS
            if target_str.starts_with("AmazonSQS") {
                return sqs_handlers::handle_request(State(state.sqs.clone()), headers, body).await;
            }
            // SNS
            if target_str.starts_with("AmazonSNS") {
                return sns_handlers::handle_request(State(state.sns.clone()), headers, body).await;
            }
            // Kinesis Firehose
            if target_str.starts_with("Firehose_") {
                return firehose_handlers::handle_request(
                    State(state.firehose.clone()),
                    headers,
                    body,
                )
                .await;
            }
        }
    }

    // Check for IAM request (uses Action parameter in body, not X-Amz-Target)
    // IAM typically uses POST with Content-Type: application/x-www-form-urlencoded
    if let Some(content_type) = headers.get(header::CONTENT_TYPE) {
        if let Ok(ct) = content_type.to_str() {
            if ct.contains("x-www-form-urlencoded") {
                // Check if body contains Action=Create/Get/Delete/List/Attach (IAM actions)
                let body_str = String::from_utf8_lossy(&body);
                if body_str.contains("Action=CreateRole")
                    || body_str.contains("Action=GetRole")
                    || body_str.contains("Action=DeleteRole")
                    || body_str.contains("Action=ListRoles")
                    || body_str.contains("Action=CreatePolicy")
                    || body_str.contains("Action=GetPolicy")
                    || body_str.contains("Action=DeletePolicy")
                    || body_str.contains("Action=AttachRolePolicy")
                    || body_str.contains("Action=DetachRolePolicy")
                    || body_str.contains("Action=ListAttachedRolePolicies")
                {
                    return iam_handlers::handle_request(State(state.iam.clone()), headers, body)
                        .await;
                }
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
    handlers::handle_bucket(
        State(state.s3.clone()),
        Path(bucket),
        method,
        Query(query),
        headers,
        body,
    )
    .await
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
    handlers::handle_object(
        State(state.s3.clone()),
        Path((bucket, key)),
        method,
        headers,
        body,
    )
    .await
}

fn service_disabled(service: &str) -> Response {
    Response::builder()
        .status(StatusCode::SERVICE_UNAVAILABLE)
        .body(Body::from(format!("Service '{}' is disabled", service)))
        .unwrap()
}

// === API Gateway V2 handlers ===

async fn apigw_create_api(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    apigateway_handlers::handle_request(
        State(state.apigateway.clone()),
        Method::POST,
        "/apis",
        headers,
        body,
    )
    .await
}

async fn apigw_list_apis(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    apigateway_handlers::handle_request(
        State(state.apigateway.clone()),
        Method::GET,
        "/apis",
        headers,
        Bytes::new(),
    )
    .await
}

async fn apigw_get_api(
    State(state): State<Arc<AppState>>,
    Path(api_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let path = format!("/apis/{}", api_id);
    apigateway_handlers::handle_request(
        State(state.apigateway.clone()),
        Method::GET,
        &path,
        headers,
        Bytes::new(),
    )
    .await
}

async fn apigw_delete_api(
    State(state): State<Arc<AppState>>,
    Path(api_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let path = format!("/apis/{}", api_id);
    apigateway_handlers::handle_request(
        State(state.apigateway.clone()),
        Method::DELETE,
        &path,
        headers,
        Bytes::new(),
    )
    .await
}

async fn apigw_create_route(
    State(state): State<Arc<AppState>>,
    Path(api_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let path = format!("/apis/{}/routes", api_id);
    apigateway_handlers::handle_request(
        State(state.apigateway.clone()),
        Method::POST,
        &path,
        headers,
        body,
    )
    .await
}

async fn apigw_list_routes(
    State(state): State<Arc<AppState>>,
    Path(api_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let path = format!("/apis/{}/routes", api_id);
    apigateway_handlers::handle_request(
        State(state.apigateway.clone()),
        Method::GET,
        &path,
        headers,
        Bytes::new(),
    )
    .await
}

async fn apigw_get_route(
    State(state): State<Arc<AppState>>,
    Path((api_id, route_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let path = format!("/apis/{}/routes/{}", api_id, route_id);
    apigateway_handlers::handle_request(
        State(state.apigateway.clone()),
        Method::GET,
        &path,
        headers,
        Bytes::new(),
    )
    .await
}

async fn apigw_delete_route(
    State(state): State<Arc<AppState>>,
    Path((api_id, route_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let path = format!("/apis/{}/routes/{}", api_id, route_id);
    apigateway_handlers::handle_request(
        State(state.apigateway.clone()),
        Method::DELETE,
        &path,
        headers,
        Bytes::new(),
    )
    .await
}

async fn apigw_create_integration(
    State(state): State<Arc<AppState>>,
    Path(api_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let path = format!("/apis/{}/integrations", api_id);
    apigateway_handlers::handle_request(
        State(state.apigateway.clone()),
        Method::POST,
        &path,
        headers,
        body,
    )
    .await
}

async fn apigw_list_integrations(
    State(state): State<Arc<AppState>>,
    Path(api_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let path = format!("/apis/{}/integrations", api_id);
    apigateway_handlers::handle_request(
        State(state.apigateway.clone()),
        Method::GET,
        &path,
        headers,
        Bytes::new(),
    )
    .await
}

async fn apigw_get_integration(
    State(state): State<Arc<AppState>>,
    Path((api_id, integration_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let path = format!("/apis/{}/integrations/{}", api_id, integration_id);
    apigateway_handlers::handle_request(
        State(state.apigateway.clone()),
        Method::GET,
        &path,
        headers,
        Bytes::new(),
    )
    .await
}

async fn apigw_delete_integration(
    State(state): State<Arc<AppState>>,
    Path((api_id, integration_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let path = format!("/apis/{}/integrations/{}", api_id, integration_id);
    apigateway_handlers::handle_request(
        State(state.apigateway.clone()),
        Method::DELETE,
        &path,
        headers,
        Bytes::new(),
    )
    .await
}

async fn apigw_create_stage(
    State(state): State<Arc<AppState>>,
    Path(api_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let path = format!("/apis/{}/stages", api_id);
    apigateway_handlers::handle_request(
        State(state.apigateway.clone()),
        Method::POST,
        &path,
        headers,
        body,
    )
    .await
}

async fn apigw_list_stages(
    State(state): State<Arc<AppState>>,
    Path(api_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let path = format!("/apis/{}/stages", api_id);
    apigateway_handlers::handle_request(
        State(state.apigateway.clone()),
        Method::GET,
        &path,
        headers,
        Bytes::new(),
    )
    .await
}

async fn apigw_get_stage(
    State(state): State<Arc<AppState>>,
    Path((api_id, stage_name)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let path = format!("/apis/{}/stages/{}", api_id, stage_name);
    apigateway_handlers::handle_request(
        State(state.apigateway.clone()),
        Method::GET,
        &path,
        headers,
        Bytes::new(),
    )
    .await
}

async fn apigw_delete_stage(
    State(state): State<Arc<AppState>>,
    Path((api_id, stage_name)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let path = format!("/apis/{}/stages/{}", api_id, stage_name);
    apigateway_handlers::handle_request(
        State(state.apigateway.clone()),
        Method::DELETE,
        &path,
        headers,
        Bytes::new(),
    )
    .await
}
