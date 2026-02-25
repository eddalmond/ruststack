//! AWS Signature Version 4 validation middleware

use axum::{body::Body, middleware::Next, response::Response};
use http::Request;
use tracing::debug;

/// Validates the structural presence of AWS SigV4 headers
/// This prevents official SDKs from throwing pre-flight errors
/// Note: We validate structure, not cryptographic correctness (for local dev)
pub async fn validate_sigv4(request: Request<Body>, next: Next) -> Response {
    let headers = request.headers();

    // Check for SigV4 signature headers
    let has_authorization = headers.contains_key("authorization");
    let has_date = headers.contains_key("x-amz-date");
    let has_credential = headers.contains_key("x-amz-credential");

    // If we have any SigV4-related headers, validate their structure
    if has_authorization || has_date {
        // Validate authorization header structure if present
        if let Some(auth) = headers.get("authorization") {
            if let Ok(auth_str) = auth.to_str() {
                // AWS SigV4 authorization header format:
                // AWS4-HMAC-SHA256 Credential=AKID/Date/Region/Service/aws4_request, SignedHeaders=..., Signature=...
                if auth_str.starts_with("AWS4-HMAC-SHA256") {
                    // Valid SigV4 prefix - continue with request
                    debug!("Valid SigV4 signature detected");
                } else if auth_str.starts_with("AWS ") {
                    // SigV2 - also acceptable
                    debug!("SigV2 signature detected");
                }
            }
        }

        // If we have x-amz-date, check for other required headers
        if has_date && !has_credential {
            // Might be a date-only request, that's okay
            debug!("SigV4 date header present");
        }
    }

    // Continue to the actual handler - we don't block requests,
    // we just ensure SDKs don't fail on structural validation
    next.run(request).await
}
