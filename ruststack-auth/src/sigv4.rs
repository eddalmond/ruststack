//! AWS Signature Version 4 implementation

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

/// Errors during signature verification
#[derive(Debug, Error)]
pub enum SigV4Error {
    #[error("Missing authorization header")]
    MissingAuthHeader,

    #[error("Invalid authorization header format")]
    InvalidAuthFormat,

    #[error("Missing credential")]
    MissingCredential,

    #[error("Invalid credential format")]
    InvalidCredentialFormat,

    #[error("Missing signed headers")]
    MissingSignedHeaders,

    #[error("Missing signature")]
    MissingSignature,

    #[error("Request time too skewed")]
    RequestTimeTooSkewed,

    #[error("Signature mismatch")]
    SignatureMismatch,
}

/// Parsed SigV4 authorization header
#[derive(Debug)]
pub struct AuthorizationHeader {
    pub algorithm: String,
    pub access_key: String,
    pub date: String,
    pub region: String,
    pub service: String,
    pub signed_headers: Vec<String>,
    pub signature: String,
}

/// Parse an AWS SigV4 authorization header
///
/// Format: AWS4-HMAC-SHA256 Credential=AKID/DATE/REGION/SERVICE/aws4_request,
///         SignedHeaders=host;x-amz-date, Signature=HEX
pub fn parse_authorization_header(header: &str) -> Result<AuthorizationHeader, SigV4Error> {
    let parts: Vec<&str> = header.splitn(2, ' ').collect();
    if parts.len() != 2 {
        return Err(SigV4Error::InvalidAuthFormat);
    }

    let algorithm = parts[0].to_string();
    let components = parts[1];

    let mut credential = None;
    let mut signed_headers = None;
    let mut signature = None;

    for component in components.split(", ") {
        let kv: Vec<&str> = component.splitn(2, '=').collect();
        if kv.len() != 2 {
            continue;
        }

        match kv[0] {
            "Credential" => credential = Some(kv[1]),
            "SignedHeaders" => signed_headers = Some(kv[1]),
            "Signature" => signature = Some(kv[1]),
            _ => {}
        }
    }

    let credential = credential.ok_or(SigV4Error::MissingCredential)?;
    let credential_parts: Vec<&str> = credential.split('/').collect();
    if credential_parts.len() != 5 {
        return Err(SigV4Error::InvalidCredentialFormat);
    }

    let signed_headers = signed_headers.ok_or(SigV4Error::MissingSignedHeaders)?;
    let signature = signature.ok_or(SigV4Error::MissingSignature)?;

    Ok(AuthorizationHeader {
        algorithm,
        access_key: credential_parts[0].to_string(),
        date: credential_parts[1].to_string(),
        region: credential_parts[2].to_string(),
        service: credential_parts[3].to_string(),
        signed_headers: signed_headers.split(';').map(String::from).collect(),
        signature: signature.to_string(),
    })
}

/// Sign a string using HMAC-SHA256
fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// Derive the signing key
fn derive_signing_key(secret_key: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = hmac_sha256(format!("AWS4{}", secret_key).as_bytes(), date.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}

/// Create the canonical request string
fn create_canonical_request(
    method: &str,
    path: &str,
    query_string: &str,
    headers: &[(String, String)],
    signed_headers: &[String],
    payload_hash: &str,
) -> String {
    let canonical_headers: String = signed_headers
        .iter()
        .filter_map(|h| {
            headers
                .iter()
                .find(|(k, _)| k.to_lowercase() == h.to_lowercase())
                .map(|(k, v)| format!("{}:{}\n", k.to_lowercase(), v.trim()))
        })
        .collect();

    let signed_headers_str = signed_headers.join(";");

    format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method, path, query_string, canonical_headers, signed_headers_str, payload_hash
    )
}

/// Create the string to sign
fn create_string_to_sign(
    algorithm: &str,
    timestamp: &str,
    scope: &str,
    canonical_request: &str,
) -> String {
    let canonical_hash = hex::encode(Sha256::digest(canonical_request.as_bytes()));
    format!(
        "{}\n{}\n{}\n{}",
        algorithm, timestamp, scope, canonical_hash
    )
}

/// Verify a SigV4 signature (placeholder - full implementation needed)
pub fn verify_signature(
    _method: &str,
    _path: &str,
    _query_string: &str,
    _headers: &[(String, String)],
    _payload: &[u8],
    _auth_header: &AuthorizationHeader,
    _secret_key: &str,
    _timestamp: &DateTime<Utc>,
) -> Result<bool, SigV4Error> {
    // TODO: Implement full verification
    // For now, accept all signatures (dev mode)
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_authorization_header() {
        let header = "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, SignedHeaders=host;range;x-amz-date, Signature=fe5f80f77d5fa3beca038a248ff027d0445342fe2855ddc963176630326f1024";

        let result = parse_authorization_header(header).unwrap();

        assert_eq!(result.algorithm, "AWS4-HMAC-SHA256");
        assert_eq!(result.access_key, "AKIAIOSFODNN7EXAMPLE");
        assert_eq!(result.date, "20130524");
        assert_eq!(result.region, "us-east-1");
        assert_eq!(result.service, "s3");
        assert_eq!(result.signed_headers, vec!["host", "range", "x-amz-date"]);
    }

    #[test]
    fn test_derive_signing_key() {
        // Test vector from AWS documentation
        let key = derive_signing_key(
            "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            "20130524",
            "us-east-1",
            "s3",
        );

        // The signing key should be deterministic
        assert!(!key.is_empty());
        assert_eq!(key.len(), 32); // SHA256 output
    }
}
