//! Cognito storage

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum CognitoError {
    #[error("User pool not found: {0}")]
    UserPoolNotFound(String),
    #[error("User not found: {0}")]
    UserNotFound(String),
    #[error("User already exists: {0}")]
    UserAlreadyExists(String),
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPool {
    pub id: String,
    pub name: String,
    pub region: String,
    pub users: HashMap<String, CognitoUser>,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub secret_key: String,
    pub created_at: DateTime<Utc>,
}

impl UserPool {
    pub fn new(name: &str, region: &str) -> Self {
        let id = format!(
            "{}_{}",
            region,
            &Uuid::new_v4().to_string().replace("-", "")[..8]
        );
        let client_id = Uuid::new_v4().to_string().replace("-", "");
        let secret_key = Uuid::new_v4().to_string();

        Self {
            id,
            name: name.to_string(),
            region: region.to_string(),
            users: HashMap::new(),
            client_id,
            client_secret: None,
            secret_key,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitoUser {
    pub username: String,
    pub password: String,
    pub email: String,
    pub email_verified: bool,
    pub enabled: bool,
    pub status: UserStatus,
    pub attributes: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum UserStatus {
    #[default]
    Unconfirmed,
    Confirmed,
    Archived,
    Compromised,
    Unknown,
    #[serde(rename = "RESET_REQUIRED")]
    ResetRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResult {
    pub id_token: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: u64,
    pub token_type: String,
}

/// State for Cognito handlers
#[derive(Debug, Default)]
pub struct CognitoState {
    user_pools: DashMap<String, UserPool>,
}

impl CognitoState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_user_pool(&self, name: &str, region: &str) -> UserPool {
        let pool = UserPool::new(name, region);
        self.user_pools.insert(pool.id.clone(), pool.clone());
        pool
    }

    pub fn get_user_pool(&self, pool_id: &str) -> Result<UserPool, CognitoError> {
        self.user_pools
            .get(pool_id)
            .map(|p| p.clone())
            .ok_or_else(|| CognitoError::UserPoolNotFound(pool_id.to_string()))
    }

    pub fn list_user_pools(&self) -> Vec<UserPool> {
        self.user_pools.iter().map(|p| p.value().clone()).collect()
    }

    pub fn create_user(
        &self,
        pool_id: &str,
        username: &str,
        password: &str,
        email: &str,
    ) -> Result<CognitoUser, CognitoError> {
        let mut pool = self.get_user_pool(pool_id)?;

        if pool.users.contains_key(username) {
            return Err(CognitoError::UserAlreadyExists(username.to_string()));
        }

        let user = CognitoUser {
            username: username.to_string(),
            password: password.to_string(),
            email: email.to_string(),
            email_verified: true,
            enabled: true,
            status: UserStatus::Confirmed,
            attributes: HashMap::from([
                ("email".to_string(), email.to_string()),
                ("email_verified".to_string(), "true".to_string()),
            ]),
            created_at: Utc::now(),
            last_modified: Utc::now(),
        };

        pool.users.insert(username.to_string(), user.clone());
        self.user_pools.insert(pool_id.to_string(), pool);

        Ok(user)
    }

    pub fn get_user(&self, pool_id: &str, username: &str) -> Result<CognitoUser, CognitoError> {
        let pool = self.get_user_pool(pool_id)?;
        pool.users
            .get(username)
            .cloned()
            .ok_or_else(|| CognitoError::UserNotFound(username.to_string()))
    }

    pub fn delete_user(&self, pool_id: &str, username: &str) -> Result<(), CognitoError> {
        let mut pool = self.get_user_pool(pool_id)?;

        if pool.users.remove(username).is_none() {
            return Err(CognitoError::UserNotFound(username.to_string()));
        }

        self.user_pools.insert(pool_id.to_string(), pool);
        Ok(())
    }

    pub fn enable_user(&self, pool_id: &str, username: &str) -> Result<CognitoUser, CognitoError> {
        let mut pool = self.get_user_pool(pool_id)?;

        let user = pool
            .users
            .get_mut(username)
            .ok_or_else(|| CognitoError::UserNotFound(username.to_string()))?;

        user.enabled = true;
        let user = user.clone();

        self.user_pools.insert(pool_id.to_string(), pool);
        Ok(user)
    }

    pub fn disable_user(&self, pool_id: &str, username: &str) -> Result<CognitoUser, CognitoError> {
        let mut pool = self.get_user_pool(pool_id)?;

        let user = pool
            .users
            .get_mut(username)
            .ok_or_else(|| CognitoError::UserNotFound(username.to_string()))?;

        user.enabled = false;
        let user = user.clone();

        self.user_pools.insert(pool_id.to_string(), pool);
        Ok(user)
    }

    pub fn authenticate(
        &self,
        pool_id: &str,
        username: &str,
        password: &str,
    ) -> Result<AuthResult, CognitoError> {
        let pool = self.get_user_pool(pool_id)?;

        let user = pool
            .users
            .get(username)
            .ok_or(CognitoError::InvalidCredentials)?;

        if user.password != password {
            return Err(CognitoError::InvalidCredentials);
        }

        if !user.enabled {
            return Err(CognitoError::InvalidCredentials);
        }

        let id_token = crate::jwt::generate_id_token(&pool, username, &user.email);
        let access_token = crate::jwt::generate_access_token(&pool, username);

        Ok(AuthResult {
            id_token,
            access_token,
            refresh_token: None,
            expires_in: 3600,
            token_type: "Bearer".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> CognitoState {
        CognitoState::new()
    }

    // === User Pool Tests ===

    #[test]
    fn test_create_user_pool() {
        let state = test_state();
        let pool = state.create_user_pool("test-pool", "us-east-1");
        
        assert_eq!(pool.name, "test-pool");
        assert_eq!(pool.region, "us-east-1");
        assert!(!pool.id.is_empty());
        assert!(pool.id.starts_with("us-east-1_"));
        assert!(!pool.client_id.is_empty());
        assert!(!pool.secret_key.is_empty());
    }

    #[test]
    fn test_create_user_pool_sets_timestamps() {
        let state = test_state();
        let pool = state.create_user_pool("timestamp-test", "eu-west-1");
        
        assert!(pool.created_at.timestamp() > 0);
    }

    #[test]
    fn test_get_user_pool() {
        let state = test_state();
        let created = state.create_user_pool("get-test", "us-west-2");
        
        let result = state.get_user_pool(&created.id);
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "get-test");
    }

    #[test]
    fn test_get_nonexistent_pool_fails() {
        let state = test_state();
        let result = state.get_user_pool("us-east-1_nonexistent");
        
        assert!(result.is_err());
        matches!(result.unwrap_err(), CognitoError::UserPoolNotFound(_));
    }

    #[test]
    fn test_list_user_pools() {
        let state = test_state();
        state.create_user_pool("pool-1", "us-east-1");
        state.create_user_pool("pool-2", "eu-west-1");
        
        let pools = state.list_user_pools();
        
        assert_eq!(pools.len(), 2);
    }

    #[test]
    fn test_list_user_pools_empty() {
        let state = test_state();
        let pools = state.list_user_pools();
        
        assert!(pools.is_empty());
    }

    #[test]
    fn test_user_pool_has_unique_id() {
        let state = test_state();
        let pool1 = state.create_user_pool("unique-test", "us-east-1");
        let pool2 = state.create_user_pool("unique-test", "us-east-1");
        
        // Different UUID suffix means different IDs
        assert_ne!(pool1.id, pool2.id);
    }

    // === User Tests ===

    #[test]
    fn test_create_user() {
        let state = test_state();
        let pool = state.create_user_pool("user-pool", "us-east-1");
        
        let user = state.create_user(
            &pool.id,
            "testuser",
            "password123",
            "test@example.com",
        );
        
        assert!(user.is_ok());
        let user = user.unwrap();
        assert_eq!(user.username, "testuser");
        assert_eq!(user.email, "test@example.com");
        assert!(user.email_verified);
        assert!(user.enabled);
    }

    #[test]
    fn test_create_user_default_status() {
        let state = test_state();
        let pool = state.create_user_pool("status-test", "us-east-1");
        
        let user = state.create_user(&pool.id, "user1", "pass", "user1@test.com").unwrap();
        
        assert_eq!(user.status, UserStatus::Confirmed);
    }

    #[test]
    fn test_create_user_adds_email_attribute() {
        let state = test_state();
        let pool = state.create_user_pool("attr-test", "us-east-1");
        
        let user = state.create_user(&pool.id, "user1", "pass", "user1@test.com").unwrap();
        
        assert_eq!(user.attributes.get("email"), Some(&"user1@test.com".to_string()));
        assert_eq!(user.attributes.get("email_verified"), Some(&"true".to_string()));
    }

    #[test]
    fn test_create_user_sets_timestamps() {
        let state = test_state();
        let pool = state.create_user_pool("time-test", "us-east-1");
        
        let user = state.create_user(&pool.id, "user1", "pass", "user1@test.com").unwrap();
        
        assert!(user.created_at.timestamp() > 0);
        assert!(user.last_modified.timestamp() > 0);
    }

    #[test]
    fn test_create_duplicate_user_fails() {
        let state = test_state();
        let pool = state.create_user_pool("dup-test", "us-east-1");
        
        state.create_user(&pool.id, "duplicate", "pass", "dup@test.com").unwrap();
        
        let result = state.create_user(&pool.id, "duplicate", "pass", "dup@test.com");
        
        assert!(result.is_err());
        matches!(result.unwrap_err(), CognitoError::UserAlreadyExists(_));
    }

    #[test]
    fn test_get_user() {
        let state = test_state();
        let pool = state.create_user_pool("get-user-test", "us-east-1");
        state.create_user(&pool.id, "getuser", "password", "getuser@test.com").unwrap();
        
        let user = state.get_user(&pool.id, "getuser");
        
        assert!(user.is_ok());
        assert_eq!(user.unwrap().username, "getuser");
    }

    #[test]
    fn test_get_nonexistent_user_fails() {
        let state = test_state();
        let pool = state.create_user_pool("nonexist-test", "us-east-1");
        
        let result = state.get_user(&pool.id, "nonexistent");
        
        assert!(result.is_err());
        matches!(result.unwrap_err(), CognitoError::UserNotFound(_));
    }

    #[test]
    fn test_delete_user() {
        let state = test_state();
        let pool = state.create_user_pool("delete-test", "us-east-1");
        state.create_user(&pool.id, "todelete", "pass", "delete@test.com").unwrap();
        
        let result = state.delete_user(&pool.id, "todelete");
        
        assert!(result.is_ok());
        
        // Verify user is gone
        let result = state.get_user(&pool.id, "todelete");
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_nonexistent_user_fails() {
        let state = test_state();
        let pool = state.create_user_pool("del-none-test", "us-east-1");
        
        let result = state.delete_user(&pool.id, "nonexistent");
        
        assert!(result.is_err());
        matches!(result.unwrap_err(), CognitoError::UserNotFound(_));
    }

    // === User Status Tests ===

    #[test]
    fn test_enable_user() {
        let state = test_state();
        let pool = state.create_user_pool("enable-test", "us-east-1");
        state.create_user(&pool.id, "user1", "pass", "user1@test.com").unwrap();
        
        // First disable
        state.disable_user(&pool.id, "user1").unwrap();
        
        // Then enable
        let user = state.enable_user(&pool.id, "user1").unwrap();
        
        assert!(user.enabled);
    }

    #[test]
    fn test_disable_user() {
        let state = test_state();
        let pool = state.create_user_pool("disable-test", "us-east-1");
        state.create_user(&pool.id, "user1", "pass", "user1@test.com").unwrap();
        
        let user = state.disable_user(&pool.id, "user1").unwrap();
        
        assert!(!user.enabled);
    }

    // === Authentication Tests ===

    #[test]
    fn test_authenticate_success() {
        let state = test_state();
        let pool = state.create_user_pool("auth-test", "us-east-1");
        state.create_user(&pool.id, "authuser", "correctpassword", "auth@test.com").unwrap();
        
        let result = state.authenticate(&pool.id, "authuser", "correctpassword");
        
        assert!(result.is_ok());
        let auth = result.unwrap();
        assert!(!auth.id_token.is_empty());
        assert!(!auth.access_token.is_empty());
        assert_eq!(auth.expires_in, 3600);
        assert_eq!(auth.token_type, "Bearer");
    }

    #[test]
    fn test_authenticate_wrong_password_fails() {
        let state = test_state();
        let pool = state.create_user_pool("auth-fail-test", "us-east-1");
        state.create_user(&pool.id, "user1", "correctpassword", "user1@test.com").unwrap();
        
        let result = state.authenticate(&pool.id, "user1", "wrongpassword");
        
        assert!(result.is_err());
        matches!(result.unwrap_err(), CognitoError::InvalidCredentials);
    }

    #[test]
    fn test_authenticate_nonexistent_user_fails() {
        let state = test_state();
        let pool = state.create_user_pool("auth-none-test", "us-east-1");
        
        let result = state.authenticate(&pool.id, "nonexistent", "password");
        
        assert!(result.is_err());
        matches!(result.unwrap_err(), CognitoError::InvalidCredentials);
    }

    #[test]
    fn test_authenticate_disabled_user_fails() {
        let state = test_state();
        let pool = state.create_user_pool("auth-disable-test", "us-east-1");
        state.create_user(&pool.id, "disableduser", "password", "disabled@test.com").unwrap();
        state.disable_user(&pool.id, "disableduser").unwrap();
        
        let result = state.authenticate(&pool.id, "disableduser", "password");
        
        assert!(result.is_err());
        matches!(result.unwrap_err(), CognitoError::InvalidCredentials);
    }

    #[test]
    fn test_authenticate_returns_tokens() {
        let state = test_state();
        let pool = state.create_user_pool("token-test", "us-east-1");
        state.create_user(&pool.id, "user1", "pass", "user1@test.com").unwrap();
        
        let auth = state.authenticate(&pool.id, "user1", "pass").unwrap();
        
        // Basic JWT structure check (should have 3 parts separated by dots)
        let id_parts: Vec<&str> = auth.id_token.split('.').collect();
        assert_eq!(id_parts.len(), 3);
        
        let access_parts: Vec<&str> = auth.access_token.split('.').collect();
        assert_eq!(access_parts.len(), 3);
    }

    // === UserStatus Enum Tests ===

    #[test]
    fn test_user_status_default() {
        let status: UserStatus = Default::default();
        assert_eq!(status, UserStatus::Unconfirmed);
    }

    #[test]
    fn test_user_status_variants() {
        let _ = UserStatus::Unconfirmed;
        let _ = UserStatus::Confirmed;
        let _ = UserStatus::Archived;
        let _ = UserStatus::Compromised;
        let _ = UserStatus::Unknown;
        let _ = UserStatus::ResetRequired;
    }
}
