//! Cognito HTTP handlers

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::Response,
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::storage::{CognitoError, CognitoState};

pub async fn handle_request(
    State(state): State<Arc<CognitoState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let target = headers
        .get("x-amz-target")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let path = headers
        .get("x-amz-path")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("/");

    tracing::debug!(target = %target, path = %path, "Cognito request");

    match target {
        // User Pool operations
        "AWSCognitoIdentityProviderService.CreateUserPool" => create_user_pool(state, body).await,
        "AWSCognitoIdentityProviderService.ListUserPools" => list_user_pools(state, body).await,
        // User operations
        "AWSCognitoIdentityProviderService.AdminCreateUser" => admin_create_user(state, body).await,
        "AWSCognitoIdentityProviderService.AdminGetUser" => admin_get_user(state, body).await,
        "AWSCognitoIdentityProviderService.AdminDeleteUser" => admin_delete_user(state, body).await,
        "AWSCognitoIdentityProviderService.AdminEnableUser" => admin_enable_user(state, body).await,
        "AWSCognitoIdentityProviderService.AdminDisableUser" => {
            admin_disable_user(state, body).await
        }
        // Auth operations
        "AWSCognitoIdentityProviderService.InitiateAuth" => initiate_auth(state, body).await,
        "AWSCognitoIdentityProviderService.AdminInitiateAuth" => {
            admin_initiate_auth(state, body).await
        }
        "AWSCognitoIdentityProviderService.GetUser" => get_user(state, body).await,
        _ => {
            tracing::warn!(target = %target, "Unknown Cognito operation");
            error_response(
                StatusCode::BAD_REQUEST,
                "UnknownOperationException",
                &format!("Unknown operation: {}", target),
            )
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct CreateUserPoolResponse {
    #[serde(rename = "UserPool")]
    user_pool: UserPoolDescription,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct UserPoolDescription {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Status")]
    status: Option<String>,
    #[serde(rename = "CreationDate")]
    creation_date: f64,
    #[serde(rename = "LastModifiedDate")]
    last_modified_date: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CreateUserPoolRequest {
    pool_name: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
struct ListUserPoolsRequest {
    max_results: Option<i32>,
    next_token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct ListUserPoolsResponse {
    #[serde(rename = "UserPools")]
    user_pools: Vec<UserPoolDescription>,
    #[serde(rename = "NextToken")]
    next_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AdminCreateUserRequest {
    user_pool_id: String,
    username: String,
    temporary_password: Option<String>,
    user_attributes: Option<Vec<UserAttribute>>,
    /// message_action: RESEND to resend verification email, SUPPRESS to skip sending
    message_action: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
struct UserAttribute {
    name: String,
    value: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct AdminCreateUserResponse {
    #[serde(rename = "User")]
    user: UserType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct UserType {
    #[serde(rename = "Username")]
    username: String,
    #[serde(rename = "UserAttributes")]
    user_attributes: Vec<UserAttribute>,
    #[serde(rename = "UserCreateDate")]
    user_create_date: f64,
    #[serde(rename = "UserLastModifiedDate")]
    user_last_modified_date: f64,
    #[serde(rename = "Enabled")]
    enabled: bool,
    #[serde(rename = "UserStatus")]
    user_status: String,
    #[serde(rename = "CreationDate")]
    creation_date: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AdminGetUserRequest {
    user_pool_id: String,
    username: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct AdminGetUserResponse {
    #[serde(rename = "Username")]
    username: String,
    #[serde(rename = "UserAttributes")]
    user_attributes: Vec<UserAttribute>,
    #[serde(rename = "UserCreateDate")]
    user_create_date: f64,
    #[serde(rename = "UserLastModifiedDate")]
    user_last_modified_date: f64,
    #[serde(rename = "Enabled")]
    enabled: bool,
    #[serde(rename = "UserStatus")]
    user_status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AdminDeleteUserRequest {
    user_pool_id: String,
    username: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AdminEnableUserRequest {
    user_pool_id: String,
    username: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AdminDisableUserRequest {
    user_pool_id: String,
    username: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct InitiateAuthRequest {
    auth_flow: String,
    auth_parameters: AuthParameters,
    #[allow(dead_code)]
    client_id: String,
    #[serde(rename = "UserPoolId")]
    user_pool_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AuthParameters {
    username: Option<String>,
    password: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct GetUserRequest {
    #[serde(rename = "AccessToken")]
    access_token: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct GetUserResponse {
    #[serde(rename = "Username")]
    username: String,
    #[serde(rename = "UserAttributes")]
    user_attributes: Vec<UserAttribute>,
    #[serde(rename = "MFAOptions")]
    mfa_options: Option<Vec<()>>,
    #[serde(rename = "PreferredMfaSetting")]
    preferred_mfa_setting: Option<String>,
    #[serde(rename = "UserMFASettingList")]
    user_mfa_setting_list: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct InitiateAuthResponse {
    #[serde(rename = "AuthenticationResult")]
    authentication_result: AuthenticationResult,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct AuthenticationResult {
    #[serde(rename = "IdToken")]
    id_token: String,
    #[serde(rename = "AccessToken")]
    access_token: String,
    #[serde(rename = "RefreshToken")]
    refresh_token: Option<String>,
    #[serde(rename = "ExpiresIn")]
    expires_in: u64,
    #[serde(rename = "TokenType")]
    token_type: String,
}

// === Handlers ===

async fn create_user_pool(state: Arc<CognitoState>, body: Bytes) -> Response {
    let req: CreateUserPoolRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationException",
                &e.to_string(),
            )
        }
    };

    let pool = state.create_user_pool(&req.pool_name, "us-east-1");

    let response = CreateUserPoolResponse {
        user_pool: UserPoolDescription {
            id: pool.id.clone(),
            name: pool.name.clone(),
            status: Some("Active".to_string()),
            creation_date: pool.created_at.timestamp() as f64,
            last_modified_date: pool.created_at.timestamp() as f64,
        },
    };

    json_response(StatusCode::OK, &response)
}

async fn list_user_pools(state: Arc<CognitoState>, body: Bytes) -> Response {
    let req: ListUserPoolsRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationException",
                &e.to_string(),
            )
        }
    };

    let max_results = req.max_results.unwrap_or(60) as usize;
    let pools = state.list_user_pools();

    let pool_descriptions: Vec<UserPoolDescription> = pools
        .into_iter()
        .take(max_results)
        .map(|p| UserPoolDescription {
            id: p.id,
            name: p.name,
            status: Some("Active".to_string()),
            creation_date: p.created_at.timestamp() as f64,
            last_modified_date: p.created_at.timestamp() as f64,
        })
        .collect();

    let response = ListUserPoolsResponse {
        user_pools: pool_descriptions,
        next_token: None,
    };

    json_response(StatusCode::OK, &response)
}

async fn admin_create_user(state: Arc<CognitoState>, body: Bytes) -> Response {
    let req: AdminCreateUserRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationException",
                &e.to_string(),
            )
        }
    };

    // Extract email from user attributes
    let email = req
        .user_attributes
        .as_ref()
        .and_then(|attrs| {
            attrs
                .iter()
                .find(|a| a.name == "email")
                .map(|a| a.value.clone())
        })
        .unwrap_or_else(|| format!("{}@example.com", req.username));

    let password = req
        .temporary_password
        .unwrap_or_else(|| "Test1234!".to_string());

    match state.create_user(&req.user_pool_id, &req.username, &password, &email) {
        Ok(user) => {
            // Handle message_action
            if let Some(action) = &req.message_action {
                match action.as_str() {
                    "RESEND" => {
                        tracing::info!(username = %req.username, pool_id = %req.user_pool_id, "Verification email resent (simulated)");
                    }
                    "SUPPRESS" => {
                        tracing::debug!(username = %req.username, pool_id = %req.user_pool_id, "User created with message_action=SUPPRESS - no verification email sent");
                    }
                    _ => {
                        tracing::warn!(action = %action, "Unknown message_action value");
                    }
                }
            }

            let user_attrs: Vec<UserAttribute> = user
                .attributes
                .iter()
                .map(|(k, v)| UserAttribute {
                    name: k.clone(),
                    value: v.clone(),
                })
                .collect();

            let response = AdminCreateUserResponse {
                user: UserType {
                    username: user.username,
                    user_attributes: user_attrs,
                    user_create_date: user.created_at.timestamp() as f64,
                    user_last_modified_date: user.last_modified.timestamp() as f64,
                    enabled: user.enabled,
                    user_status: format!("{:?}", user.status),
                    creation_date: user.created_at.timestamp() as f64,
                },
            };

            json_response(StatusCode::OK, &response)
        }
        Err(CognitoError::UserAlreadyExists(_)) => error_response(
            StatusCode::BAD_REQUEST,
            "UsernameExistsException",
            "User already exists",
        ),
        Err(CognitoError::UserPoolNotFound(_)) => error_response(
            StatusCode::BAD_REQUEST,
            "ResourceNotFoundException",
            "User pool not found",
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            &e.to_string(),
        ),
    }
}

async fn admin_get_user(state: Arc<CognitoState>, body: Bytes) -> Response {
    let req: AdminGetUserRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationException",
                &e.to_string(),
            )
        }
    };

    match state.get_user(&req.user_pool_id, &req.username) {
        Ok(user) => {
            let user_attrs: Vec<UserAttribute> = user
                .attributes
                .iter()
                .map(|(k, v)| UserAttribute {
                    name: k.clone(),
                    value: v.clone(),
                })
                .collect();

            let response = AdminGetUserResponse {
                username: user.username,
                user_attributes: user_attrs,
                user_create_date: user.created_at.timestamp() as f64,
                user_last_modified_date: user.last_modified.timestamp() as f64,
                enabled: user.enabled,
                user_status: format!("{:?}", user.status),
            };

            json_response(StatusCode::OK, &response)
        }
        Err(CognitoError::UserNotFound(_)) => error_response(
            StatusCode::BAD_REQUEST,
            "UserNotFoundException",
            "User does not exist",
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            &e.to_string(),
        ),
    }
}

async fn admin_delete_user(state: Arc<CognitoState>, body: Bytes) -> Response {
    let req: AdminDeleteUserRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationException",
                &e.to_string(),
            )
        }
    };

    match state.delete_user(&req.user_pool_id, &req.username) {
        Ok(_) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))
            .unwrap(),
        Err(CognitoError::UserNotFound(_)) => error_response(
            StatusCode::BAD_REQUEST,
            "UserNotFoundException",
            "User does not exist",
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            &e.to_string(),
        ),
    }
}

async fn admin_enable_user(state: Arc<CognitoState>, body: Bytes) -> Response {
    let req: AdminEnableUserRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationException",
                &e.to_string(),
            )
        }
    };

    match state.enable_user(&req.user_pool_id, &req.username) {
        Ok(_user) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))
            .unwrap(),
        Err(CognitoError::UserNotFound(_)) => error_response(
            StatusCode::NOT_FOUND,
            "UserNotFoundException",
            "User does not exist",
        ),
        Err(CognitoError::UserPoolNotFound(_)) => error_response(
            StatusCode::NOT_FOUND,
            "ResourceNotFoundException",
            "User pool not found",
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            &e.to_string(),
        ),
    }
}

async fn admin_disable_user(state: Arc<CognitoState>, body: Bytes) -> Response {
    let req: AdminDisableUserRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationException",
                &e.to_string(),
            )
        }
    };

    match state.disable_user(&req.user_pool_id, &req.username) {
        Ok(_user) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))
            .unwrap(),
        Err(CognitoError::UserNotFound(_)) => error_response(
            StatusCode::NOT_FOUND,
            "UserNotFoundException",
            "User does not exist",
        ),
        Err(CognitoError::UserPoolNotFound(_)) => error_response(
            StatusCode::NOT_FOUND,
            "ResourceNotFoundException",
            "User pool not found",
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            &e.to_string(),
        ),
    }
}

async fn initiate_auth(state: Arc<CognitoState>, body: Bytes) -> Response {
    let req: InitiateAuthRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationException",
                &e.to_string(),
            )
        }
    };

    let valid_flows = [
        "USER_SRP_AUTH",
        "USER_PASSWORD_AUTH",
        "REFRESH_TOKEN_AUTH",
        "CUSTOM_AUTH",
    ];
    if !valid_flows.contains(&req.auth_flow.as_str()) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "InvalidParameterException",
            "Invalid authentication flow type",
        );
    }

    let username = req
        .auth_parameters
        .username
        .ok_or_else(|| "Username required".to_string())
        .unwrap();
    let password = req
        .auth_parameters
        .password
        .ok_or_else(|| "Password required".to_string())
        .unwrap();

    // For now, we need a user pool ID - let's use the first one if not provided
    let pool_id = req.user_pool_id.unwrap_or_else(|| {
        state
            .list_user_pools()
            .first()
            .map(|p| p.id.clone())
            .unwrap_or_default()
    });

    if pool_id.is_empty() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "UserPoolNotFoundException",
            "No user pool found",
        );
    }

    // Validate client_id if provided
    let pool = match state.get_user_pool(&pool_id) {
        Ok(p) => p,
        Err(CognitoError::UserPoolNotFound(_)) => {
            return error_response(
                StatusCode::NOT_FOUND,
                "ResourceNotFoundException",
                "User pool not found",
            )
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "InternalError",
                &e.to_string(),
            )
        }
    };

    if !req.client_id.is_empty() && req.client_id != pool.client_id {
        return error_response(
            StatusCode::BAD_REQUEST,
            "InvalidParameterException",
            "Client_id does not match",
        );
    }

    match state.authenticate(&pool_id, &username, &password) {
        Ok(auth_result) => {
            let response = InitiateAuthResponse {
                authentication_result: AuthenticationResult {
                    id_token: auth_result.id_token,
                    access_token: auth_result.access_token,
                    refresh_token: auth_result.refresh_token,
                    expires_in: auth_result.expires_in,
                    token_type: auth_result.token_type,
                },
            };
            json_response(StatusCode::OK, &response)
        }
        Err(CognitoError::InvalidCredentials) => error_response(
            StatusCode::BAD_REQUEST,
            "NotAuthorizedException",
            "Incorrect username or password",
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            &e.to_string(),
        ),
    }
}

async fn admin_initiate_auth(state: Arc<CognitoState>, body: Bytes) -> Response {
    // Same as initiate_auth for now
    initiate_auth(state, body).await
}

async fn get_user(state: Arc<CognitoState>, body: Bytes) -> Response {
    let req: GetUserRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationException",
                &e.to_string(),
            )
        }
    };

    // Extract pool_id from the token or find first pool
    let pools = state.list_user_pools();
    if pools.is_empty() {
        return error_response(
            StatusCode::NOT_FOUND,
            "UserNotFoundException",
            "No user pools found",
        );
    }

    // Try to get pool_id from token claims first, otherwise use first pool
    let pool_id = pools[0].id.clone();
    let secret_key = pools[0].secret_key.clone();

    // Decode and verify the access token
    let token_data = match crate::jwt::verify_token(&req.access_token, &secret_key) {
        Ok(data) => data,
        Err(_) => {
            return error_response(
                StatusCode::UNAUTHORIZED,
                "NotAuthorizedException",
                "Invalid access token",
            )
        }
    };

    // Extract username from token claims
    let claims = token_data.claims;
    let username = match claims.get("username").and_then(|v| v.as_str()) {
        Some(u) => u.to_string(),
        None => {
            return error_response(
                StatusCode::UNAUTHORIZED,
                "NotAuthorizedException",
                "Invalid token claims",
            )
        }
    };

    // Get user from the pool
    let user = match state.get_user(&pool_id, &username) {
        Ok(u) => u,
        Err(CognitoError::UserNotFound(_)) => {
            return error_response(
                StatusCode::NOT_FOUND,
                "UserNotFoundException",
                "User not found",
            )
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "InternalError",
                &e.to_string(),
            )
        }
    };

    let user_attrs: Vec<UserAttribute> = user
        .attributes
        .iter()
        .map(|(k, v)| UserAttribute {
            name: k.clone(),
            value: v.clone(),
        })
        .collect();

    let response = GetUserResponse {
        username: user.username,
        user_attributes: user_attrs,
        mfa_options: None,
        preferred_mfa_setting: None,
        user_mfa_setting_list: None,
    };

    json_response(StatusCode::OK, &response)
}

// === Helpers ===

fn json_response<T: serde::Serialize>(status: StatusCode, body: &T) -> Response {
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
