//! JWT token generation for Cognito

use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header};
use serde::{Deserialize, Serialize};

use crate::storage::UserPool;

#[derive(Serialize)]
struct IdTokenClaims {
    sub: String,
    iss: String,
    #[serde(rename = "cognito:username")]
    cognito_username: String,
    origin_jti: String,
    aud: String,
    event_id: String,
    #[serde(rename = "token_use")]
    token_use: String,
    #[serde(rename = "auth_time")]
    auth_time: u64,
    exp: u64,
    iat: u64,
    jti: String,
    email: String,
    #[serde(rename = "email_verified")]
    email_verified: bool,
    #[serde(rename = "cognito:user_status")]
    cognito_user_status: String,
    #[serde(rename = "cognito:user_pool_id")]
    cognito_user_pool_id: String,
}

#[derive(Serialize, Deserialize)]
struct AccessTokenClaims {
    sub: String,
    iss: String,
    #[serde(rename = "client_id")]
    client_id: String,
    origin_jti: String,
    event_id: String,
    #[serde(rename = "token_use")]
    token_use: String,
    scope: String,
    exp: u64,
    iat: u64,
    jti: String,
    username: String,
    #[serde(rename = "auth_time")]
    auth_time: u64,
    #[serde(rename = "cognito:user_pool_id")]
    cognito_user_pool_id: String,
}

pub fn generate_id_token(pool: &UserPool, username: &str, email: &str) -> String {
    let now = Utc::now().timestamp() as u64;
    let exp = now + 3600;

    let claims = IdTokenClaims {
        sub: username.to_string(),
        iss: format!(
            "https://cognito-idp.{}.amazonaws.com/{}",
            pool.region, pool.id
        ),
        cognito_username: username.to_string(),
        origin_jti: uuid::Uuid::new_v4().to_string(),
        aud: pool.client_id.clone(),
        event_id: uuid::Uuid::new_v4().to_string(),
        token_use: "id".to_string(),
        auth_time: now,
        exp,
        iat: now,
        jti: uuid::Uuid::new_v4().to_string(),
        email: email.to_string(),
        email_verified: true,
        cognito_user_status: "CONFIRMED".to_string(),
        cognito_user_pool_id: pool.id.clone(),
    };

    let header = Header::new(Algorithm::HS256);
    encode(
        &header,
        &claims,
        &EncodingKey::from_secret(pool.secret_key.as_bytes()),
    )
    .unwrap_or_default()
}

pub fn generate_access_token(pool: &UserPool, username: &str) -> String {
    let now = Utc::now().timestamp() as u64;
    let exp = now + 3600;

    let claims = AccessTokenClaims {
        sub: username.to_string(),
        iss: format!(
            "https://cognito-idp.{}.amazonaws.com/{}",
            pool.region, pool.id
        ),
        client_id: pool.client_id.clone(),
        origin_jti: uuid::Uuid::new_v4().to_string(),
        event_id: uuid::Uuid::new_v4().to_string(),
        token_use: "access".to_string(),
        scope: "aws.cognito.signin.user.admin".to_string(),
        exp,
        iat: now,
        jti: uuid::Uuid::new_v4().to_string(),
        username: username.to_string(),
        auth_time: now,
        cognito_user_pool_id: pool.id.clone(),
    };

    let header = Header::new(Algorithm::HS256);
    encode(
        &header,
        &claims,
        &EncodingKey::from_secret(pool.secret_key.as_bytes()),
    )
    .unwrap_or_default()
}

#[allow(dead_code)]
pub fn verify_token(
    token: &str,
    secret: &str,
) -> Result<jsonwebtoken::TokenData<serde_json::Value>, ()> {
    let decode = decode::<serde_json::Value>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &jsonwebtoken::Validation::new(Algorithm::HS256),
    );

    decode.map_err(|_| ())
}
