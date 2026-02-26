//! IAM Middleware for RustStack services
//!
//! This module provides utilities for enforcing IAM policies
//! on incoming requests.

use crate::policy::{EvaluationContext, PolicyEngine};
use axum::body::Body;
use axum::http::StatusCode;
use axum::response::Response;

/// Check if IAM enforcement is enabled
pub fn is_iam_enforced() -> bool {
    std::env::var("ENFORCE_IAM")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
}

/// Evaluate a request against IAM policies
/// Returns Some(403 response) if denied, None if allowed
pub fn evaluate_request(
    policy_engine: &PolicyEngine,
    principal_arn: Option<String>,
    action: &str,
    resource: &str,
    _service: &str,
) -> Option<Response<Body>> {
    if !is_iam_enforced() {
        return None;
    }

    let context = EvaluationContext {
        principal_arn,
        action: action.to_string(),
        resource_arn: resource.to_string(),
        service: _service.to_string(),
    };

    // For now, we don't have policies loaded - return None (allowed)
    // Full implementation would look up attached policies and evaluate
    let _context = context;
    let _policy_engine = policy_engine;

    None
}

/// Build an access denied error response
pub fn access_denied_error(action: &str, resource: &str) -> Response<Body> {
    let error = format_error_xml(action, resource);
    Response::builder()
        .status(StatusCode::FORBIDDEN)
        .header("Content-Type", "application/xml")
        .body(Body::from(error))
        .unwrap()
}

fn format_error_xml(action: &str, resource: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<AccessDenied xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Message>Access Denied</Message>
  <Action>{}</Action>
  <Resource>{}</Resource>
</AccessDenied>"#,
        action, resource
    )
}
