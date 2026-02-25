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
