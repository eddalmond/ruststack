//! HTTP handlers for Secrets Manager

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::Response,
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

use crate::storage::{SecretsManagerError, SecretsManagerState};

/// Handle Secrets Manager requests based on X-Amz-Target header
pub async fn handle_request(
    State(state): State<Arc<SecretsManagerState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let target = headers
        .get("x-amz-target")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    info!(target = %target, "Secrets Manager request");

    match target {
        "secretsmanager.CreateSecret" => handle_create_secret(state, body).await,
        "secretsmanager.GetSecretValue" => handle_get_secret_value(state, body).await,
        "secretsmanager.PutSecretValue" => handle_put_secret_value(state, body).await,
        "secretsmanager.DeleteSecret" => handle_delete_secret(state, body).await,
        "secretsmanager.DescribeSecret" => handle_describe_secret(state, body).await,
        "secretsmanager.ListSecrets" => handle_list_secrets(state, body).await,
        _ => {
            warn!(target = %target, "Unknown Secrets Manager operation");
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
struct CreateSecretRequest {
    name: String,
    description: Option<String>,
    kms_key_id: Option<String>,
    secret_string: Option<String>,
    secret_binary: Option<String>,
    #[serde(default)]
    tags: Vec<Tag>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Tag {
    key: String,
    value: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct CreateSecretResponse {
    #[serde(rename = "ARN")]
    arn: String,
    name: String,
    version_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct GetSecretValueRequest {
    secret_id: String,
    version_id: Option<String>,
    version_stage: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct GetSecretValueResponse {
    #[serde(rename = "ARN")]
    arn: String,
    name: String,
    version_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    secret_string: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    secret_binary: Option<String>,
    version_stages: Vec<String>,
    created_date: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PutSecretValueRequest {
    secret_id: String,
    secret_string: Option<String>,
    secret_binary: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct PutSecretValueResponse {
    #[serde(rename = "ARN")]
    arn: String,
    name: String,
    version_id: String,
    version_stages: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
struct DeleteSecretRequest {
    secret_id: String,
    #[serde(default)]
    force_delete_without_recovery: bool,
    recovery_window_in_days: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct DeleteSecretResponse {
    #[serde(rename = "ARN")]
    arn: String,
    name: String,
    deletion_date: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DescribeSecretRequest {
    secret_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct DescribeSecretResponse {
    #[serde(rename = "ARN")]
    arn: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kms_key_id: Option<String>,
    created_date: f64,
    last_changed_date: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_accessed_date: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deleted_date: Option<f64>,
    version_ids_to_stages: HashMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
struct ListSecretsRequest {
    max_results: Option<i32>,
    next_token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct ListSecretsResponse {
    secret_list: Vec<SecretListEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct SecretListEntry {
    #[serde(rename = "ARN")]
    arn: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    created_date: f64,
    last_changed_date: f64,
}

// === Handlers ===

async fn handle_create_secret(state: Arc<SecretsManagerState>, body: Bytes) -> Response {
    let req: CreateSecretRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationException",
                &e.to_string(),
            )
        }
    };

    let tags: HashMap<String, String> = req.tags.into_iter().map(|t| (t.key, t.value)).collect();

    match state.storage.create_secret(
        &req.name,
        req.description,
        req.kms_key_id,
        req.secret_string,
        req.secret_binary,
        tags,
    ) {
        Ok(secret) => {
            let response = CreateSecretResponse {
                arn: secret.arn,
                name: secret.name,
                version_id: secret.current_version_id,
            };
            json_response(StatusCode::OK, &response)
        }
        Err(SecretsManagerError::ResourceExists(name)) => error_response(
            StatusCode::BAD_REQUEST,
            "ResourceExistsException",
            &format!("Secret {} already exists", name),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalServiceError",
            &e.to_string(),
        ),
    }
}

async fn handle_get_secret_value(state: Arc<SecretsManagerState>, body: Bytes) -> Response {
    let req: GetSecretValueRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationException",
                &e.to_string(),
            )
        }
    };

    match state.storage.get_secret_value(
        &req.secret_id,
        req.version_id.as_deref(),
        req.version_stage.as_deref(),
    ) {
        Ok((secret, version)) => {
            let response = GetSecretValueResponse {
                arn: secret.arn,
                name: secret.name,
                version_id: version.version_id,
                secret_string: version.secret_string,
                secret_binary: version.secret_binary,
                version_stages: version.version_stages,
                created_date: version.created_date.timestamp() as f64,
            };
            json_response(StatusCode::OK, &response)
        }
        Err(SecretsManagerError::ResourceNotFound(id)) => error_response(
            StatusCode::BAD_REQUEST,
            "ResourceNotFoundException",
            &format!("Secret {} not found", id),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalServiceError",
            &e.to_string(),
        ),
    }
}

async fn handle_put_secret_value(state: Arc<SecretsManagerState>, body: Bytes) -> Response {
    let req: PutSecretValueRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationException",
                &e.to_string(),
            )
        }
    };

    match state
        .storage
        .put_secret_value(&req.secret_id, req.secret_string, req.secret_binary)
    {
        Ok((secret, version)) => {
            let response = PutSecretValueResponse {
                arn: secret.arn,
                name: secret.name,
                version_id: version.version_id,
                version_stages: version.version_stages,
            };
            json_response(StatusCode::OK, &response)
        }
        Err(SecretsManagerError::ResourceNotFound(id)) => error_response(
            StatusCode::BAD_REQUEST,
            "ResourceNotFoundException",
            &format!("Secret {} not found", id),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalServiceError",
            &e.to_string(),
        ),
    }
}

async fn handle_delete_secret(state: Arc<SecretsManagerState>, body: Bytes) -> Response {
    let req: DeleteSecretRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationException",
                &e.to_string(),
            )
        }
    };

    match state
        .storage
        .delete_secret(&req.secret_id, req.force_delete_without_recovery)
    {
        Ok(secret) => {
            let response = DeleteSecretResponse {
                arn: secret.arn,
                name: secret.name,
                deletion_date: secret.deleted_date.map(|d| d.timestamp() as f64),
            };
            json_response(StatusCode::OK, &response)
        }
        Err(SecretsManagerError::ResourceNotFound(id)) => error_response(
            StatusCode::BAD_REQUEST,
            "ResourceNotFoundException",
            &format!("Secret {} not found", id),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalServiceError",
            &e.to_string(),
        ),
    }
}

async fn handle_describe_secret(state: Arc<SecretsManagerState>, body: Bytes) -> Response {
    let req: DescribeSecretRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationException",
                &e.to_string(),
            )
        }
    };

    match state.storage.describe_secret(&req.secret_id) {
        Ok(secret) => {
            let version_ids_to_stages: HashMap<String, Vec<String>> = secret
                .versions
                .iter()
                .map(|(k, v)| (k.clone(), v.version_stages.clone()))
                .collect();

            let response = DescribeSecretResponse {
                arn: secret.arn,
                name: secret.name,
                description: secret.description,
                kms_key_id: secret.kms_key_id,
                created_date: secret.created_date.timestamp() as f64,
                last_changed_date: secret.last_changed_date.timestamp() as f64,
                last_accessed_date: secret.last_accessed_date.map(|d| d.timestamp() as f64),
                deleted_date: secret.deleted_date.map(|d| d.timestamp() as f64),
                version_ids_to_stages,
            };
            json_response(StatusCode::OK, &response)
        }
        Err(SecretsManagerError::ResourceNotFound(id)) => error_response(
            StatusCode::BAD_REQUEST,
            "ResourceNotFoundException",
            &format!("Secret {} not found", id),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalServiceError",
            &e.to_string(),
        ),
    }
}

async fn handle_list_secrets(state: Arc<SecretsManagerState>, body: Bytes) -> Response {
    let _req: ListSecretsRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => ListSecretsRequest {
            max_results: None,
            next_token: None,
        },
    };

    let secrets = state.storage.list_secrets();
    let secret_list: Vec<SecretListEntry> = secrets
        .into_iter()
        .map(|s| SecretListEntry {
            arn: s.arn,
            name: s.name,
            description: s.description,
            created_date: s.created_date.timestamp() as f64,
            last_changed_date: s.last_changed_date.timestamp() as f64,
        })
        .collect();

    let response = ListSecretsResponse {
        secret_list,
        next_token: None,
    };
    json_response(StatusCode::OK, &response)
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
