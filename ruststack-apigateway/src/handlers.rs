//! HTTP handlers for API Gateway V2

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, Method, StatusCode},
    response::Response,
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

use crate::storage::ApiGatewayState;

/// Handle API Gateway V2 requests
/// Routes are REST-style: POST /apis, GET /apis/{apiId}, etc.
pub async fn handle_request(
    State(state): State<Arc<ApiGatewayState>>,
    method: Method,
    path: &str,
    _headers: HeaderMap,
    body: Bytes,
) -> Response {
    info!(method = %method, path = %path, "API Gateway request");

    // Parse the path to determine the operation
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();

    match (method.clone(), parts.as_slice()) {
        // APIs
        (Method::POST, ["apis"]) => handle_create_api(state, body).await,
        (Method::GET, ["apis"]) => handle_list_apis(state).await,
        (Method::GET, ["apis", api_id]) => handle_get_api(state, api_id).await,
        (Method::DELETE, ["apis", api_id]) => handle_delete_api(state, api_id).await,

        // Routes
        (Method::POST, ["apis", api_id, "routes"]) => handle_create_route(state, api_id, body).await,
        (Method::GET, ["apis", api_id, "routes"]) => handle_list_routes(state, api_id).await,
        (Method::GET, ["apis", api_id, "routes", route_id]) => handle_get_route(state, api_id, route_id).await,
        (Method::DELETE, ["apis", api_id, "routes", route_id]) => handle_delete_route(state, api_id, route_id).await,

        // Integrations
        (Method::POST, ["apis", api_id, "integrations"]) => handle_create_integration(state, api_id, body).await,
        (Method::GET, ["apis", api_id, "integrations"]) => handle_list_integrations(state, api_id).await,
        (Method::GET, ["apis", api_id, "integrations", int_id]) => handle_get_integration(state, api_id, int_id).await,
        (Method::DELETE, ["apis", api_id, "integrations", int_id]) => handle_delete_integration(state, api_id, int_id).await,

        // Stages
        (Method::POST, ["apis", api_id, "stages"]) => handle_create_stage(state, api_id, body).await,
        (Method::GET, ["apis", api_id, "stages"]) => handle_list_stages(state, api_id).await,
        (Method::GET, ["apis", api_id, "stages", stage_name]) => handle_get_stage(state, api_id, stage_name).await,
        (Method::DELETE, ["apis", api_id, "stages", stage_name]) => handle_delete_stage(state, api_id, stage_name).await,

        _ => {
            warn!(method = %method, path = %path, "Unknown API Gateway operation");
            error_response(StatusCode::NOT_FOUND, "NotFoundException", &format!("Unknown path: {}", path))
        }
    }
}

// === Request/Response types ===

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CreateApiRequest {
    name: String,
    protocol_type: String,
    description: Option<String>,
    #[serde(default)]
    tags: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct ApiResponse {
    api_id: String,
    name: String,
    protocol_type: String,
    api_endpoint: String,
    created_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CreateRouteRequest {
    route_key: String,
    target: Option<String>,
    authorization_type: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct RouteResponse {
    route_id: String,
    route_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    authorization_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CreateIntegrationRequest {
    integration_type: String,
    integration_uri: Option<String>,
    integration_method: Option<String>,
    payload_format_version: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct IntegrationResponse {
    integration_id: String,
    integration_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    integration_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    integration_method: Option<String>,
    payload_format_version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CreateStageRequest {
    stage_name: String,
    #[serde(default)]
    auto_deploy: bool,
    description: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct StageResponse {
    stage_name: String,
    auto_deploy: bool,
    created_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

// === API Handlers ===

async fn handle_create_api(state: Arc<ApiGatewayState>, body: Bytes) -> Response {
    let req: CreateApiRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, "BadRequestException", &e.to_string()),
    };

    let api = state.storage.create_api(&req.name, &req.protocol_type, req.description, req.tags);

    let response = ApiResponse {
        api_id: api.api_id,
        name: api.name,
        protocol_type: api.protocol_type,
        api_endpoint: api.api_endpoint,
        created_date: api.created_date.to_rfc3339(),
        description: api.description,
    };
    json_response(StatusCode::CREATED, &response)
}

async fn handle_get_api(state: Arc<ApiGatewayState>, api_id: &str) -> Response {
    match state.storage.get_api(api_id) {
        Some(api) => {
            let response = ApiResponse {
                api_id: api.api_id,
                name: api.name,
                protocol_type: api.protocol_type,
                api_endpoint: api.api_endpoint,
                created_date: api.created_date.to_rfc3339(),
                description: api.description,
            };
            json_response(StatusCode::OK, &response)
        }
        None => error_response(StatusCode::NOT_FOUND, "NotFoundException", &format!("API {} not found", api_id)),
    }
}

async fn handle_delete_api(state: Arc<ApiGatewayState>, api_id: &str) -> Response {
    match state.storage.delete_api(api_id) {
        Some(_) => Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Body::empty())
            .unwrap(),
        None => error_response(StatusCode::NOT_FOUND, "NotFoundException", &format!("API {} not found", api_id)),
    }
}

async fn handle_list_apis(state: Arc<ApiGatewayState>) -> Response {
    let apis = state.storage.list_apis();
    let items: Vec<ApiResponse> = apis
        .into_iter()
        .map(|api| ApiResponse {
            api_id: api.api_id,
            name: api.name,
            protocol_type: api.protocol_type,
            api_endpoint: api.api_endpoint,
            created_date: api.created_date.to_rfc3339(),
            description: api.description,
        })
        .collect();

    let response = serde_json::json!({ "Items": items });
    json_response(StatusCode::OK, &response)
}

// === Route Handlers ===

async fn handle_create_route(state: Arc<ApiGatewayState>, api_id: &str, body: Bytes) -> Response {
    let req: CreateRouteRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, "BadRequestException", &e.to_string()),
    };

    match state.storage.create_route(api_id, &req.route_key, req.target) {
        Some(route) => {
            let response = RouteResponse {
                route_id: route.route_id,
                route_key: route.route_key,
                target: route.target,
                authorization_type: route.authorization_type,
            };
            json_response(StatusCode::CREATED, &response)
        }
        None => error_response(StatusCode::NOT_FOUND, "NotFoundException", &format!("API {} not found", api_id)),
    }
}

async fn handle_get_route(state: Arc<ApiGatewayState>, api_id: &str, route_id: &str) -> Response {
    match state.storage.get_route(api_id, route_id) {
        Some(route) => {
            let response = RouteResponse {
                route_id: route.route_id,
                route_key: route.route_key,
                target: route.target,
                authorization_type: route.authorization_type,
            };
            json_response(StatusCode::OK, &response)
        }
        None => error_response(StatusCode::NOT_FOUND, "NotFoundException", "Route not found"),
    }
}

async fn handle_delete_route(state: Arc<ApiGatewayState>, api_id: &str, route_id: &str) -> Response {
    match state.storage.delete_route(api_id, route_id) {
        Some(_) => Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Body::empty())
            .unwrap(),
        None => error_response(StatusCode::NOT_FOUND, "NotFoundException", "Route not found"),
    }
}

async fn handle_list_routes(state: Arc<ApiGatewayState>, api_id: &str) -> Response {
    let routes = state.storage.list_routes(api_id);
    let items: Vec<RouteResponse> = routes
        .into_iter()
        .map(|r| RouteResponse {
            route_id: r.route_id,
            route_key: r.route_key,
            target: r.target,
            authorization_type: r.authorization_type,
        })
        .collect();

    let response = serde_json::json!({ "Items": items });
    json_response(StatusCode::OK, &response)
}

// === Integration Handlers ===

async fn handle_create_integration(state: Arc<ApiGatewayState>, api_id: &str, body: Bytes) -> Response {
    let req: CreateIntegrationRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, "BadRequestException", &e.to_string()),
    };

    match state.storage.create_integration(
        api_id,
        &req.integration_type,
        req.integration_uri,
        req.integration_method,
        req.payload_format_version,
    ) {
        Some(integration) => {
            let response = IntegrationResponse {
                integration_id: integration.integration_id,
                integration_type: integration.integration_type,
                integration_uri: integration.integration_uri,
                integration_method: integration.integration_method,
                payload_format_version: integration.payload_format_version,
            };
            json_response(StatusCode::CREATED, &response)
        }
        None => error_response(StatusCode::NOT_FOUND, "NotFoundException", &format!("API {} not found", api_id)),
    }
}

async fn handle_get_integration(state: Arc<ApiGatewayState>, api_id: &str, integration_id: &str) -> Response {
    match state.storage.get_integration(api_id, integration_id) {
        Some(integration) => {
            let response = IntegrationResponse {
                integration_id: integration.integration_id,
                integration_type: integration.integration_type,
                integration_uri: integration.integration_uri,
                integration_method: integration.integration_method,
                payload_format_version: integration.payload_format_version,
            };
            json_response(StatusCode::OK, &response)
        }
        None => error_response(StatusCode::NOT_FOUND, "NotFoundException", "Integration not found"),
    }
}

async fn handle_delete_integration(state: Arc<ApiGatewayState>, api_id: &str, integration_id: &str) -> Response {
    match state.storage.delete_integration(api_id, integration_id) {
        Some(_) => Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Body::empty())
            .unwrap(),
        None => error_response(StatusCode::NOT_FOUND, "NotFoundException", "Integration not found"),
    }
}

async fn handle_list_integrations(state: Arc<ApiGatewayState>, api_id: &str) -> Response {
    let integrations = state.storage.list_integrations(api_id);
    let items: Vec<IntegrationResponse> = integrations
        .into_iter()
        .map(|i| IntegrationResponse {
            integration_id: i.integration_id,
            integration_type: i.integration_type,
            integration_uri: i.integration_uri,
            integration_method: i.integration_method,
            payload_format_version: i.payload_format_version,
        })
        .collect();

    let response = serde_json::json!({ "Items": items });
    json_response(StatusCode::OK, &response)
}

// === Stage Handlers ===

async fn handle_create_stage(state: Arc<ApiGatewayState>, api_id: &str, body: Bytes) -> Response {
    let req: CreateStageRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, "BadRequestException", &e.to_string()),
    };

    match state.storage.create_stage(api_id, &req.stage_name, req.auto_deploy, req.description) {
        Some(stage) => {
            let response = StageResponse {
                stage_name: stage.stage_name,
                auto_deploy: stage.auto_deploy,
                created_date: stage.created_date.to_rfc3339(),
                description: stage.description,
            };
            json_response(StatusCode::CREATED, &response)
        }
        None => error_response(StatusCode::NOT_FOUND, "NotFoundException", &format!("API {} not found", api_id)),
    }
}

async fn handle_get_stage(state: Arc<ApiGatewayState>, api_id: &str, stage_name: &str) -> Response {
    match state.storage.get_stage(api_id, stage_name) {
        Some(stage) => {
            let response = StageResponse {
                stage_name: stage.stage_name,
                auto_deploy: stage.auto_deploy,
                created_date: stage.created_date.to_rfc3339(),
                description: stage.description,
            };
            json_response(StatusCode::OK, &response)
        }
        None => error_response(StatusCode::NOT_FOUND, "NotFoundException", "Stage not found"),
    }
}

async fn handle_delete_stage(state: Arc<ApiGatewayState>, api_id: &str, stage_name: &str) -> Response {
    match state.storage.delete_stage(api_id, stage_name) {
        Some(_) => Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Body::empty())
            .unwrap(),
        None => error_response(StatusCode::NOT_FOUND, "NotFoundException", "Stage not found"),
    }
}

async fn handle_list_stages(state: Arc<ApiGatewayState>, api_id: &str) -> Response {
    let stages = state.storage.list_stages(api_id);
    let items: Vec<StageResponse> = stages
        .into_iter()
        .map(|s| StageResponse {
            stage_name: s.stage_name,
            auto_deploy: s.auto_deploy,
            created_date: s.created_date.to_rfc3339(),
            description: s.description,
        })
        .collect();

    let response = serde_json::json!({ "Items": items });
    json_response(StatusCode::OK, &response)
}

// === Helpers ===

fn json_response<T: Serialize>(status: StatusCode, body: &T) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
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
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}
