//! Secrets Manager in-memory storage

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// A secret with its versions
#[derive(Debug, Clone)]
pub struct Secret {
    /// Secret ARN
    pub arn: String,
    /// Secret name
    pub name: String,
    /// Description
    pub description: Option<String>,
    /// KMS Key ID (optional, not actually used for encryption)
    pub kms_key_id: Option<String>,
    /// Secret versions (version_id -> SecretVersion)
    pub versions: HashMap<String, SecretVersion>,
    /// Current version ID (AWSCURRENT)
    pub current_version_id: Option<String>,
    /// Previous version ID (AWSPREVIOUS)
    pub previous_version_id: Option<String>,
    /// Creation date
    pub created_date: DateTime<Utc>,
    /// Last changed date
    pub last_changed_date: DateTime<Utc>,
    /// Last accessed date
    pub last_accessed_date: Option<DateTime<Utc>>,
    /// Deletion date (if scheduled for deletion)
    pub deleted_date: Option<DateTime<Utc>>,
    /// Tags
    pub tags: HashMap<String, String>,
}

/// A version of a secret
#[derive(Debug, Clone)]
pub struct SecretVersion {
    /// Version ID
    pub version_id: String,
    /// Secret string value
    pub secret_string: Option<String>,
    /// Secret binary value (base64 encoded)
    pub secret_binary: Option<String>,
    /// Creation date
    pub created_date: DateTime<Utc>,
    /// Version stages (e.g., AWSCURRENT, AWSPREVIOUS)
    pub version_stages: Vec<String>,
}

/// In-memory storage for secrets
#[derive(Debug, Default)]
pub struct SecretsManagerStorage {
    /// Secrets indexed by name
    secrets: DashMap<String, Secret>,
}

impl SecretsManagerStorage {
    pub fn new() -> Self {
        Self {
            secrets: DashMap::new(),
        }
    }

    /// Create a new secret
    pub fn create_secret(
        &self,
        name: &str,
        description: Option<String>,
        kms_key_id: Option<String>,
        secret_string: Option<String>,
        secret_binary: Option<String>,
        tags: HashMap<String, String>,
    ) -> Result<Secret, SecretsManagerError> {
        if self.secrets.contains_key(name) {
            return Err(SecretsManagerError::ResourceExists(name.to_string()));
        }

        let now = Utc::now();
        let version_id = Uuid::new_v4().to_string();
        let arn = format!(
            "arn:aws:secretsmanager:us-east-1:000000000000:secret:{}-{}",
            name,
            &Uuid::new_v4().to_string()[..6]
        );

        let mut versions = HashMap::new();
        let current_version_id = if secret_string.is_some() || secret_binary.is_some() {
            versions.insert(
                version_id.clone(),
                SecretVersion {
                    version_id: version_id.clone(),
                    secret_string,
                    secret_binary,
                    created_date: now,
                    version_stages: vec!["AWSCURRENT".to_string()],
                },
            );
            Some(version_id)
        } else {
            None
        };

        let secret = Secret {
            arn,
            name: name.to_string(),
            description,
            kms_key_id,
            versions,
            current_version_id,
            previous_version_id: None,
            created_date: now,
            last_changed_date: now,
            last_accessed_date: None,
            deleted_date: None,
            tags,
        };

        self.secrets.insert(name.to_string(), secret.clone());
        Ok(secret)
    }

    /// Get a secret by name
    pub fn get_secret(&self, name: &str) -> Result<Secret, SecretsManagerError> {
        self.secrets
            .get(name)
            .map(|s| s.clone())
            .ok_or_else(|| SecretsManagerError::ResourceNotFound(name.to_string()))
    }

    /// Get secret value
    pub fn get_secret_value(
        &self,
        secret_id: &str,
        version_id: Option<&str>,
        version_stage: Option<&str>,
    ) -> Result<(Secret, SecretVersion), SecretsManagerError> {
        let mut secret = self
            .secrets
            .get_mut(secret_id)
            .ok_or_else(|| SecretsManagerError::ResourceNotFound(secret_id.to_string()))?;

        // Update last accessed date
        secret.last_accessed_date = Some(Utc::now());

        let version_id = if let Some(vid) = version_id {
            vid.to_string()
        } else if let Some(stage) = version_stage {
            match stage {
                "AWSCURRENT" => secret
                    .current_version_id
                    .clone()
                    .ok_or(SecretsManagerError::ResourceNotFound(
                        "No current version".to_string(),
                    ))?,
                "AWSPREVIOUS" => secret
                    .previous_version_id
                    .clone()
                    .ok_or(SecretsManagerError::ResourceNotFound(
                        "No previous version".to_string(),
                    ))?,
                _ => {
                    return Err(SecretsManagerError::InvalidParameter(format!(
                        "Unknown version stage: {}",
                        stage
                    )))
                }
            }
        } else {
            secret
                .current_version_id
                .clone()
                .ok_or(SecretsManagerError::ResourceNotFound(
                    "No current version".to_string(),
                ))?
        };

        let version = secret
            .versions
            .get(&version_id)
            .cloned()
            .ok_or(SecretsManagerError::ResourceNotFound(format!(
                "Version {} not found",
                version_id
            )))?;

        Ok((secret.clone(), version))
    }

    /// Put a new secret value
    pub fn put_secret_value(
        &self,
        secret_id: &str,
        secret_string: Option<String>,
        secret_binary: Option<String>,
    ) -> Result<(Secret, SecretVersion), SecretsManagerError> {
        let mut secret = self
            .secrets
            .get_mut(secret_id)
            .ok_or_else(|| SecretsManagerError::ResourceNotFound(secret_id.to_string()))?;

        let now = Utc::now();
        let new_version_id = Uuid::new_v4().to_string();

        // Clone the IDs we need before mutating
        let current_vid = secret.current_version_id.clone();
        let prev_vid = secret.previous_version_id.clone();

        // Move current to previous
        if let Some(ref cvid) = current_vid {
            if let Some(current_version) = secret.versions.get_mut(cvid) {
                current_version.version_stages.retain(|s| s != "AWSCURRENT");
                current_version.version_stages.push("AWSPREVIOUS".to_string());
            }
            // Remove AWSPREVIOUS from old previous
            if let Some(ref pvid) = prev_vid {
                if let Some(prev_version) = secret.versions.get_mut(pvid) {
                    prev_version.version_stages.retain(|s| s != "AWSPREVIOUS");
                }
            }
            secret.previous_version_id = Some(cvid.clone());
        }

        // Create new version
        let new_version = SecretVersion {
            version_id: new_version_id.clone(),
            secret_string,
            secret_binary,
            created_date: now,
            version_stages: vec!["AWSCURRENT".to_string()],
        };

        secret.versions.insert(new_version_id.clone(), new_version.clone());
        secret.current_version_id = Some(new_version_id);
        secret.last_changed_date = now;

        Ok((secret.clone(), new_version))
    }

    /// Delete a secret
    pub fn delete_secret(
        &self,
        secret_id: &str,
        force_delete: bool,
    ) -> Result<Secret, SecretsManagerError> {
        if force_delete {
            self.secrets
                .remove(secret_id)
                .map(|(_, s)| s)
                .ok_or_else(|| SecretsManagerError::ResourceNotFound(secret_id.to_string()))
        } else {
            // Schedule for deletion (we'll just mark it)
            let mut secret = self
                .secrets
                .get_mut(secret_id)
                .ok_or_else(|| SecretsManagerError::ResourceNotFound(secret_id.to_string()))?;
            secret.deleted_date = Some(Utc::now() + chrono::Duration::days(30));
            Ok(secret.clone())
        }
    }

    /// List all secrets
    pub fn list_secrets(&self) -> Vec<Secret> {
        self.secrets.iter().map(|r| r.value().clone()).collect()
    }

    /// Describe a secret
    pub fn describe_secret(&self, secret_id: &str) -> Result<Secret, SecretsManagerError> {
        self.get_secret(secret_id)
    }
}

/// Secrets Manager errors
#[derive(Debug, thiserror::Error)]
pub enum SecretsManagerError {
    #[error("Secret already exists: {0}")]
    ResourceExists(String),

    #[error("Secret not found: {0}")]
    ResourceNotFound(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Decryption failure")]
    DecryptionFailure,
}

/// State for Secrets Manager handlers
pub struct SecretsManagerState {
    pub storage: Arc<SecretsManagerStorage>,
}

impl SecretsManagerState {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(SecretsManagerStorage::new()),
        }
    }
}

impl Default for SecretsManagerState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get_secret() {
        let storage = SecretsManagerStorage::new();
        
        let secret = storage
            .create_secret(
                "my-secret",
                Some("Test secret".to_string()),
                None,
                Some("secret-value".to_string()),
                None,
                HashMap::new(),
            )
            .unwrap();

        assert_eq!(secret.name, "my-secret");
        assert!(secret.current_version_id.is_some());

        let (retrieved, version) = storage
            .get_secret_value("my-secret", None, None)
            .unwrap();
        
        assert_eq!(retrieved.name, "my-secret");
        assert_eq!(version.secret_string, Some("secret-value".to_string()));
    }

    #[test]
    fn test_put_secret_value_rotates_versions() {
        let storage = SecretsManagerStorage::new();
        
        storage
            .create_secret(
                "my-secret",
                None,
                None,
                Some("value-1".to_string()),
                None,
                HashMap::new(),
            )
            .unwrap();

        // Put new value
        storage
            .put_secret_value("my-secret", Some("value-2".to_string()), None)
            .unwrap();

        // Current should be value-2
        let (_, current) = storage
            .get_secret_value("my-secret", None, Some("AWSCURRENT"))
            .unwrap();
        assert_eq!(current.secret_string, Some("value-2".to_string()));

        // Previous should be value-1
        let (_, previous) = storage
            .get_secret_value("my-secret", None, Some("AWSPREVIOUS"))
            .unwrap();
        assert_eq!(previous.secret_string, Some("value-1".to_string()));
    }

    #[test]
    fn test_duplicate_secret_fails() {
        let storage = SecretsManagerStorage::new();
        
        storage
            .create_secret("my-secret", None, None, None, None, HashMap::new())
            .unwrap();

        let result = storage.create_secret("my-secret", None, None, None, None, HashMap::new());
        assert!(matches!(result, Err(SecretsManagerError::ResourceExists(_))));
    }

    #[test]
    fn test_get_nonexistent_secret_fails() {
        let storage = SecretsManagerStorage::new();
        
        let result = storage.get_secret_value("nonexistent", None, None);
        assert!(matches!(result, Err(SecretsManagerError::ResourceNotFound(_))));
    }
}
