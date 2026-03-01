//! IAM Middleware for Request Enforcement

use axum::{
    body::{Body, to_bytes},
    extract::Request,
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::collections::HashMap;

use crate::{IamState, IamStorage, PolicyDocument, EvaluationContext, PolicyEngine, Decision};
use tracing::{debug, warn};

/// Check whether IAM should be enforced locally based on the config
pub fn is_iam_enforced() -> bool {
    std::env::var("RUSTSTACK_ENFORCE_IAM")
        .or_else(|_| std::env::var("ENFORCE_IAM"))
        .map(|s| s == "1" || s.to_lowercase() == "true")
        .unwrap_or(false)
}

/// Formats a standard access denied JSON structure
pub fn access_denied_error(message: &str) -> Response {
    let body = format!(
        r#"{{"__type":"AccessDeniedException","message":"{}"}}"#,
        message
    );
    Response::builder()
        .status(StatusCode::FORBIDDEN)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap()
}

/// The Axum middleware entry point to enforce AWS IAM policies
pub async fn enforce_iam(
    request: Request<Body>,
    next: Next,
) -> Response {
    if !is_iam_enforced() {
        return next.run(request).await;
    }

    let uri_path = request.uri().path().to_string();
    let method_str = request.method().as_str();

    // Skip Healthchecks
    if uri_path == "/health" || uri_path == "/_localstack/health" {
        return next.run(request).await;
    }

    // Try extracting AWS credentials from Authorization header First
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    let (access_key, action, resource) = extract_metadata(&request);

    // Provide default debug action maps for some localstack SDK integrations
    let determined_action = match &action {
        Some(a) => a.clone(),
        None => guess_action_from_uri(method_str, &uri_path)
    };

    // Evaluate credentials
    if let Some(ak) = &access_key {
        debug!(access_key = %ak, action = %determined_action, resource = %resource, "Evaluating IAM Request");
        
        let state = request.extensions().get::<std::sync::Arc<crate::storage::IamState>>();
        if let Some(iam_state) = state {
           if let Ok(decision) = evaluate_request(iam_state, ak, &determined_action, &resource).await {
               if decision != Decision::Allow {
                   warn!(action=%determined_action, resource=%resource, "Access Denied by IAM evaluation");
                   return access_denied_error("Access Denied: Explicit Deny or Implicit Deny in policy eval");
               }
           }
        }
    } else {
        warn!(action=%determined_action, resource=%resource, "Access Denied: Unsigned request");
        return access_denied_error("Access Denied: Make sure to sign your request with SigV4");
    }

    next.run(request).await
}

async fn evaluate_request(state: &IamState, access_key: &str, action: &str, resource: &str) -> Result<Decision, ()> {
    // For local simulation, access keys tie straight to 'roles' via a mocked association
    // Real IAM would trace AccessKey -> User -> Groups/Policies or AccessKey -> Role (AssumeRole)
    // To cleanly integrate with local testing (where RoleName == AccessKey or "test" overrides)
    
    if access_key == "test" && !std::env::var("RUSTSTACK_STRICT_IAM").map(|v| v=="1").unwrap_or(false) {
        // "test" acts as root by default unless RUSTSTACK_STRICT_IAM is set
        return Ok(Decision::Allow);
    }
    
    let storage = &state.storage;
    let roles = storage.list_roles();
    
    // Attempt mapping the simple AccessKey to the RoleName
    let matching_role = roles.into_iter().find(|r| r.role_name == access_key || r.role_id == access_key);
    
    if let Some(role) = matching_role {
        let mut policy_docs = Vec::new();
        // Load all attached policies
        for p_arn in &role.attached_policies {
            if let Ok(pol) = storage.get_policy(p_arn) {
               if let Ok(doc) = PolicyDocument::from_json(&pol.policy_document) {
                   policy_docs.push(doc);
               } 
            }
        }
        
        let conditions = HashMap::new();
        let ctx = EvaluationContext {
            action,
            resource,
            principal_arn: Some(&role.arn),
            conditions: &conditions,
        };
        
        return Ok(PolicyEngine::evaluate(&policy_docs, &ctx));
    }
    
    Ok(Decision::ImplicitDeny)
}

fn extract_metadata(request: &Request<Body>) -> (Option<String>, Option<String>, String) {
    let mut access_key = None;
    let mut action = None;
    let mut resource = "*".to_string(); // ARN fallback

    // Attempt SigV4 header extraction
    if let Some(auth) = request.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        if auth.starts_with("AWS4-HMAC-SHA256 ") {
            if let Some(cred_part) = auth.split("Credential=").nth(1) {
                if let Some(key) = cred_part.split('/').next() {
                    access_key = Some(key.to_string());
                }
            }
        }
    }

    // Attempt X-Amz-Target for Action (Dynamo, Firehose, SNS, SQS post bodies)
    if let Some(target) = request.headers().get("X-Amz-Target").and_then(|v| v.to_str().ok()) {
        // Format: DynamoDB_20120810.CreateTable -> dynamodb:CreateTable
        let parts: Vec<&str> = target.split('.').collect();
        if parts.len() == 2 {
            let service = parts[0].split('_').next().unwrap_or("").to_lowercase();
            let op = parts[1];
            action = Some(format!("{}:{}", service, op));
        }
    }

    // Try to extract S3 bucket from Host
    if let Some(host) = request.headers().get("Host").and_then(|v| v.to_str().ok()) {
        if host.contains(".localhost") || host.contains(".s3.localstack") {
            let bucket = host.split('.').next().unwrap_or("");
            if !bucket.is_empty() {
                resource = format!("arn:aws:s3:::{}", bucket);
            }
        }
    }

    (access_key, action, resource)
}

fn guess_action_from_uri(method: &str, uri: &str) -> String {
    // Basic heuristics for remaining services not using X-Amz-Target
    if uri.starts_with("/2015-03-31/functions") {
        return format!("lambda:{}", match method {
            "POST" => "CreateFunction",
            "GET" => "GetFunction",
            "DELETE" => "DeleteFunction",
            "PUT" => "UpdateFunctionCode", // or config
            _ => "Invoke"
        });
    }

    if uri.starts_with("/v2/apis") {
        return "apigateway:*".to_string();
    }

    // Default to a generic S3 action for the root mappings
    format!("s3:{}Object", match method {
        "GET" => "Get",
        "PUT" => "Put",
        "DELETE" => "Delete",
        _ => "List"
    })
}
