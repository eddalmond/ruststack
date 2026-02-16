//! HTTP handlers for IAM

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::Response,
};
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

use crate::storage::{IamError, IamState};

/// Handle IAM requests
/// IAM uses query string action parameter, not X-Amz-Target
pub async fn handle_request(
    State(state): State<Arc<IamState>>,
    _headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Parse form-urlencoded body
    let params: HashMap<String, String> = form_urlencoded::parse(&body)
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let action = params.get("Action").map(|s| s.as_str()).unwrap_or("");

    info!(action = %action, "IAM request");

    match action {
        "CreateRole" => handle_create_role(state, params).await,
        "GetRole" => handle_get_role(state, params).await,
        "DeleteRole" => handle_delete_role(state, params).await,
        "ListRoles" => handle_list_roles(state).await,
        "CreatePolicy" => handle_create_policy(state, params).await,
        "GetPolicy" => handle_get_policy(state, params).await,
        "DeletePolicy" => handle_delete_policy(state, params).await,
        "AttachRolePolicy" => handle_attach_role_policy(state, params).await,
        "DetachRolePolicy" => handle_detach_role_policy(state, params).await,
        "ListAttachedRolePolicies" => handle_list_attached_role_policies(state, params).await,
        _ => {
            warn!(action = %action, "Unknown IAM operation");
            error_response(
                StatusCode::BAD_REQUEST,
                "InvalidAction",
                &format!("Unknown action: {}", action),
            )
        }
    }
}

// === Handlers ===

async fn handle_create_role(state: Arc<IamState>, params: HashMap<String, String>) -> Response {
    let role_name = match params.get("RoleName") {
        Some(n) => n,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                "RoleName is required",
            )
        }
    };
    let assume_role_policy = params
        .get("AssumeRolePolicyDocument")
        .map(|s| s.as_str())
        .unwrap_or("{}");
    let description = params.get("Description").cloned();
    let path = params.get("Path").cloned();

    match state
        .storage
        .create_role(role_name, assume_role_policy, description, path)
    {
        Ok(role) => {
            let xml = format!(
                r#"<CreateRoleResponse xmlns="https://iam.amazonaws.com/doc/2010-05-08/">
  <CreateRoleResult>
    <Role>
      <RoleName>{}</RoleName>
      <RoleId>{}</RoleId>
      <Arn>{}</Arn>
      <Path>{}</Path>
      <CreateDate>{}</CreateDate>
    </Role>
  </CreateRoleResult>
  <ResponseMetadata>
    <RequestId>{}</RequestId>
  </ResponseMetadata>
</CreateRoleResponse>"#,
                role.role_name,
                role.role_id,
                role.arn,
                role.path,
                role.create_date.format("%Y-%m-%dT%H:%M:%SZ"),
                uuid::Uuid::new_v4()
            );
            xml_response(StatusCode::OK, &xml)
        }
        Err(IamError::EntityAlreadyExists(name)) => error_response(
            StatusCode::CONFLICT,
            "EntityAlreadyExists",
            &format!("Role {} already exists", name),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "ServiceFailure",
            &e.to_string(),
        ),
    }
}

async fn handle_get_role(state: Arc<IamState>, params: HashMap<String, String>) -> Response {
    let role_name = match params.get("RoleName") {
        Some(n) => n,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                "RoleName is required",
            )
        }
    };

    match state.storage.get_role(role_name) {
        Ok(role) => {
            let xml = format!(
                r#"<GetRoleResponse xmlns="https://iam.amazonaws.com/doc/2010-05-08/">
  <GetRoleResult>
    <Role>
      <RoleName>{}</RoleName>
      <RoleId>{}</RoleId>
      <Arn>{}</Arn>
      <Path>{}</Path>
      <CreateDate>{}</CreateDate>
    </Role>
  </GetRoleResult>
  <ResponseMetadata>
    <RequestId>{}</RequestId>
  </ResponseMetadata>
</GetRoleResponse>"#,
                role.role_name,
                role.role_id,
                role.arn,
                role.path,
                role.create_date.format("%Y-%m-%dT%H:%M:%SZ"),
                uuid::Uuid::new_v4()
            );
            xml_response(StatusCode::OK, &xml)
        }
        Err(IamError::NoSuchEntity(name)) => error_response(
            StatusCode::NOT_FOUND,
            "NoSuchEntity",
            &format!("Role {} not found", name),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "ServiceFailure",
            &e.to_string(),
        ),
    }
}

async fn handle_delete_role(state: Arc<IamState>, params: HashMap<String, String>) -> Response {
    let role_name = match params.get("RoleName") {
        Some(n) => n,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                "RoleName is required",
            )
        }
    };

    match state.storage.delete_role(role_name) {
        Ok(()) => {
            let xml = format!(
                r#"<DeleteRoleResponse xmlns="https://iam.amazonaws.com/doc/2010-05-08/">
  <ResponseMetadata>
    <RequestId>{}</RequestId>
  </ResponseMetadata>
</DeleteRoleResponse>"#,
                uuid::Uuid::new_v4()
            );
            xml_response(StatusCode::OK, &xml)
        }
        Err(IamError::NoSuchEntity(name)) => error_response(
            StatusCode::NOT_FOUND,
            "NoSuchEntity",
            &format!("Role {} not found", name),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "ServiceFailure",
            &e.to_string(),
        ),
    }
}

async fn handle_list_roles(state: Arc<IamState>) -> Response {
    let roles = state.storage.list_roles();
    let roles_xml: String = roles
        .iter()
        .map(|r| {
            format!(
                r#"    <member>
      <RoleName>{}</RoleName>
      <RoleId>{}</RoleId>
      <Arn>{}</Arn>
      <Path>{}</Path>
      <CreateDate>{}</CreateDate>
    </member>"#,
                r.role_name,
                r.role_id,
                r.arn,
                r.path,
                r.create_date.format("%Y-%m-%dT%H:%M:%SZ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let xml = format!(
        r#"<ListRolesResponse xmlns="https://iam.amazonaws.com/doc/2010-05-08/">
  <ListRolesResult>
    <Roles>
{}
    </Roles>
    <IsTruncated>false</IsTruncated>
  </ListRolesResult>
  <ResponseMetadata>
    <RequestId>{}</RequestId>
  </ResponseMetadata>
</ListRolesResponse>"#,
        roles_xml,
        uuid::Uuid::new_v4()
    );
    xml_response(StatusCode::OK, &xml)
}

async fn handle_create_policy(state: Arc<IamState>, params: HashMap<String, String>) -> Response {
    let policy_name = match params.get("PolicyName") {
        Some(n) => n,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                "PolicyName is required",
            )
        }
    };
    let policy_document = params
        .get("PolicyDocument")
        .map(|s| s.as_str())
        .unwrap_or("{}");
    let description = params.get("Description").cloned();
    let path = params.get("Path").cloned();

    match state
        .storage
        .create_policy(policy_name, policy_document, description, path)
    {
        Ok(policy) => {
            let xml = format!(
                r#"<CreatePolicyResponse xmlns="https://iam.amazonaws.com/doc/2010-05-08/">
  <CreatePolicyResult>
    <Policy>
      <PolicyName>{}</PolicyName>
      <PolicyId>{}</PolicyId>
      <Arn>{}</Arn>
      <Path>{}</Path>
      <CreateDate>{}</CreateDate>
      <AttachmentCount>{}</AttachmentCount>
    </Policy>
  </CreatePolicyResult>
  <ResponseMetadata>
    <RequestId>{}</RequestId>
  </ResponseMetadata>
</CreatePolicyResponse>"#,
                policy.policy_name,
                policy.policy_id,
                policy.arn,
                policy.path,
                policy.create_date.format("%Y-%m-%dT%H:%M:%SZ"),
                policy.attachment_count,
                uuid::Uuid::new_v4()
            );
            xml_response(StatusCode::OK, &xml)
        }
        Err(IamError::EntityAlreadyExists(name)) => error_response(
            StatusCode::CONFLICT,
            "EntityAlreadyExists",
            &format!("Policy {} already exists", name),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "ServiceFailure",
            &e.to_string(),
        ),
    }
}

async fn handle_get_policy(state: Arc<IamState>, params: HashMap<String, String>) -> Response {
    let policy_arn = match params.get("PolicyArn") {
        Some(n) => n,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                "PolicyArn is required",
            )
        }
    };

    match state.storage.get_policy(policy_arn) {
        Ok(policy) => {
            let xml = format!(
                r#"<GetPolicyResponse xmlns="https://iam.amazonaws.com/doc/2010-05-08/">
  <GetPolicyResult>
    <Policy>
      <PolicyName>{}</PolicyName>
      <PolicyId>{}</PolicyId>
      <Arn>{}</Arn>
      <Path>{}</Path>
      <CreateDate>{}</CreateDate>
      <AttachmentCount>{}</AttachmentCount>
    </Policy>
  </GetPolicyResult>
  <ResponseMetadata>
    <RequestId>{}</RequestId>
  </ResponseMetadata>
</GetPolicyResponse>"#,
                policy.policy_name,
                policy.policy_id,
                policy.arn,
                policy.path,
                policy.create_date.format("%Y-%m-%dT%H:%M:%SZ"),
                policy.attachment_count,
                uuid::Uuid::new_v4()
            );
            xml_response(StatusCode::OK, &xml)
        }
        Err(IamError::NoSuchEntity(arn)) => error_response(
            StatusCode::NOT_FOUND,
            "NoSuchEntity",
            &format!("Policy {} not found", arn),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "ServiceFailure",
            &e.to_string(),
        ),
    }
}

async fn handle_delete_policy(state: Arc<IamState>, params: HashMap<String, String>) -> Response {
    let policy_arn = match params.get("PolicyArn") {
        Some(n) => n,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                "PolicyArn is required",
            )
        }
    };

    match state.storage.delete_policy(policy_arn) {
        Ok(()) => {
            let xml = format!(
                r#"<DeletePolicyResponse xmlns="https://iam.amazonaws.com/doc/2010-05-08/">
  <ResponseMetadata>
    <RequestId>{}</RequestId>
  </ResponseMetadata>
</DeletePolicyResponse>"#,
                uuid::Uuid::new_v4()
            );
            xml_response(StatusCode::OK, &xml)
        }
        Err(IamError::NoSuchEntity(arn)) => error_response(
            StatusCode::NOT_FOUND,
            "NoSuchEntity",
            &format!("Policy {} not found", arn),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "ServiceFailure",
            &e.to_string(),
        ),
    }
}

async fn handle_attach_role_policy(
    state: Arc<IamState>,
    params: HashMap<String, String>,
) -> Response {
    let role_name = match params.get("RoleName") {
        Some(n) => n,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                "RoleName is required",
            )
        }
    };
    let policy_arn = match params.get("PolicyArn") {
        Some(n) => n,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                "PolicyArn is required",
            )
        }
    };

    match state.storage.attach_role_policy(role_name, policy_arn) {
        Ok(()) => {
            let xml = format!(
                r#"<AttachRolePolicyResponse xmlns="https://iam.amazonaws.com/doc/2010-05-08/">
  <ResponseMetadata>
    <RequestId>{}</RequestId>
  </ResponseMetadata>
</AttachRolePolicyResponse>"#,
                uuid::Uuid::new_v4()
            );
            xml_response(StatusCode::OK, &xml)
        }
        Err(IamError::NoSuchEntity(name)) => error_response(
            StatusCode::NOT_FOUND,
            "NoSuchEntity",
            &format!("Entity {} not found", name),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "ServiceFailure",
            &e.to_string(),
        ),
    }
}

async fn handle_detach_role_policy(
    state: Arc<IamState>,
    params: HashMap<String, String>,
) -> Response {
    let role_name = match params.get("RoleName") {
        Some(n) => n,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                "RoleName is required",
            )
        }
    };
    let policy_arn = match params.get("PolicyArn") {
        Some(n) => n,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                "PolicyArn is required",
            )
        }
    };

    match state.storage.detach_role_policy(role_name, policy_arn) {
        Ok(()) => {
            let xml = format!(
                r#"<DetachRolePolicyResponse xmlns="https://iam.amazonaws.com/doc/2010-05-08/">
  <ResponseMetadata>
    <RequestId>{}</RequestId>
  </ResponseMetadata>
</DetachRolePolicyResponse>"#,
                uuid::Uuid::new_v4()
            );
            xml_response(StatusCode::OK, &xml)
        }
        Err(IamError::NoSuchEntity(name)) => error_response(
            StatusCode::NOT_FOUND,
            "NoSuchEntity",
            &format!("Entity {} not found", name),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "ServiceFailure",
            &e.to_string(),
        ),
    }
}

async fn handle_list_attached_role_policies(
    state: Arc<IamState>,
    params: HashMap<String, String>,
) -> Response {
    let role_name = match params.get("RoleName") {
        Some(n) => n,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                "RoleName is required",
            )
        }
    };

    match state.storage.list_attached_role_policies(role_name) {
        Ok(policies) => {
            let policies_xml: String = policies
                .iter()
                .map(|arn| {
                    // Extract policy name from ARN
                    let name = arn.split('/').next_back().unwrap_or(arn);
                    format!(
                        r#"    <member>
      <PolicyName>{}</PolicyName>
      <PolicyArn>{}</PolicyArn>
    </member>"#,
                        name, arn
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            let xml = format!(
                r#"<ListAttachedRolePoliciesResponse xmlns="https://iam.amazonaws.com/doc/2010-05-08/">
  <ListAttachedRolePoliciesResult>
    <AttachedPolicies>
{}
    </AttachedPolicies>
    <IsTruncated>false</IsTruncated>
  </ListAttachedRolePoliciesResult>
  <ResponseMetadata>
    <RequestId>{}</RequestId>
  </ResponseMetadata>
</ListAttachedRolePoliciesResponse>"#,
                policies_xml,
                uuid::Uuid::new_v4()
            );
            xml_response(StatusCode::OK, &xml)
        }
        Err(IamError::NoSuchEntity(name)) => error_response(
            StatusCode::NOT_FOUND,
            "NoSuchEntity",
            &format!("Role {} not found", name),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "ServiceFailure",
            &e.to_string(),
        ),
    }
}

// === Helpers ===

fn xml_response(status: StatusCode, body: &str) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/xml; charset=utf-8")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn error_response(status: StatusCode, error_code: &str, message: &str) -> Response {
    let xml = format!(
        r#"<ErrorResponse xmlns="https://iam.amazonaws.com/doc/2010-05-08/">
  <Error>
    <Code>{}</Code>
    <Message>{}</Message>
  </Error>
  <RequestId>{}</RequestId>
</ErrorResponse>"#,
        error_code,
        message,
        uuid::Uuid::new_v4()
    );
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/xml; charset=utf-8")
        .body(Body::from(xml))
        .unwrap()
}
