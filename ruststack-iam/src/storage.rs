//! IAM in-memory storage (stub implementation)

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;

/// An IAM Role
#[derive(Debug, Clone)]
pub struct Role {
    /// Role name
    pub role_name: String,
    /// Role ID
    pub role_id: String,
    /// Role ARN
    pub arn: String,
    /// Path
    pub path: String,
    /// Assume role policy document (JSON string)
    pub assume_role_policy_document: String,
    /// Description
    pub description: Option<String>,
    /// Creation date
    pub create_date: DateTime<Utc>,
    /// Attached policy ARNs
    pub attached_policies: Vec<String>,
}

/// An IAM Policy
#[derive(Debug, Clone)]
pub struct Policy {
    /// Policy name
    pub policy_name: String,
    /// Policy ID
    pub policy_id: String,
    /// Policy ARN
    pub arn: String,
    /// Path
    pub path: String,
    /// Policy document (JSON string)
    pub policy_document: String,
    /// Description
    pub description: Option<String>,
    /// Creation date
    pub create_date: DateTime<Utc>,
    /// Attachment count
    pub attachment_count: i32,
}

/// In-memory IAM storage
#[derive(Debug, Default)]
pub struct IamStorage {
    /// Roles indexed by name
    roles: DashMap<String, Role>,
    /// Policies indexed by ARN
    policies: DashMap<String, Policy>,
}

impl IamStorage {
    pub fn new() -> Self {
        Self {
            roles: DashMap::new(),
            policies: DashMap::new(),
        }
    }

    /// Create a role
    pub fn create_role(
        &self,
        role_name: &str,
        assume_role_policy_document: &str,
        description: Option<String>,
        path: Option<String>,
    ) -> Result<Role, IamError> {
        if self.roles.contains_key(role_name) {
            return Err(IamError::EntityAlreadyExists(role_name.to_string()));
        }

        let path = path.unwrap_or_else(|| "/".to_string());
        let role_id = format!(
            "AROA{}",
            &Uuid::new_v4().to_string().replace("-", "")[..17].to_uppercase()
        );
        let arn = format!("arn:aws:iam::000000000000:role{}{}", path, role_name);

        let role = Role {
            role_name: role_name.to_string(),
            role_id,
            arn,
            path,
            assume_role_policy_document: assume_role_policy_document.to_string(),
            description,
            create_date: Utc::now(),
            attached_policies: Vec::new(),
        };

        self.roles.insert(role_name.to_string(), role.clone());
        Ok(role)
    }

    /// Get a role
    pub fn get_role(&self, role_name: &str) -> Result<Role, IamError> {
        self.roles
            .get(role_name)
            .map(|r| r.clone())
            .ok_or_else(|| IamError::NoSuchEntity(role_name.to_string()))
    }

    /// Delete a role
    pub fn delete_role(&self, role_name: &str) -> Result<(), IamError> {
        self.roles
            .remove(role_name)
            .map(|_| ())
            .ok_or_else(|| IamError::NoSuchEntity(role_name.to_string()))
    }

    /// List roles
    pub fn list_roles(&self) -> Vec<Role> {
        self.roles.iter().map(|r| r.value().clone()).collect()
    }

    /// Create a policy
    pub fn create_policy(
        &self,
        policy_name: &str,
        policy_document: &str,
        description: Option<String>,
        path: Option<String>,
    ) -> Result<Policy, IamError> {
        let path = path.unwrap_or_else(|| "/".to_string());
        let policy_id = format!(
            "ANPA{}",
            &Uuid::new_v4().to_string().replace("-", "")[..17].to_uppercase()
        );
        let arn = format!("arn:aws:iam::000000000000:policy{}{}", path, policy_name);

        if self.policies.contains_key(&arn) {
            return Err(IamError::EntityAlreadyExists(policy_name.to_string()));
        }

        let policy = Policy {
            policy_name: policy_name.to_string(),
            policy_id,
            arn: arn.clone(),
            path,
            policy_document: policy_document.to_string(),
            description,
            create_date: Utc::now(),
            attachment_count: 0,
        };

        self.policies.insert(arn, policy.clone());
        Ok(policy)
    }

    /// Get a policy by ARN
    pub fn get_policy(&self, policy_arn: &str) -> Result<Policy, IamError> {
        self.policies
            .get(policy_arn)
            .map(|p| p.clone())
            .ok_or_else(|| IamError::NoSuchEntity(policy_arn.to_string()))
    }

    /// Delete a policy
    pub fn delete_policy(&self, policy_arn: &str) -> Result<(), IamError> {
        self.policies
            .remove(policy_arn)
            .map(|_| ())
            .ok_or_else(|| IamError::NoSuchEntity(policy_arn.to_string()))
    }

    /// Attach a policy to a role
    pub fn attach_role_policy(&self, role_name: &str, policy_arn: &str) -> Result<(), IamError> {
        let mut role = self
            .roles
            .get_mut(role_name)
            .ok_or_else(|| IamError::NoSuchEntity(role_name.to_string()))?;

        if !role.attached_policies.contains(&policy_arn.to_string()) {
            role.attached_policies.push(policy_arn.to_string());

            // Increment attachment count on policy
            if let Some(mut policy) = self.policies.get_mut(policy_arn) {
                policy.attachment_count += 1;
            }
        }
        Ok(())
    }

    /// Detach a policy from a role
    pub fn detach_role_policy(&self, role_name: &str, policy_arn: &str) -> Result<(), IamError> {
        let mut role = self
            .roles
            .get_mut(role_name)
            .ok_or_else(|| IamError::NoSuchEntity(role_name.to_string()))?;

        role.attached_policies.retain(|p| p != policy_arn);

        // Decrement attachment count on policy
        if let Some(mut policy) = self.policies.get_mut(policy_arn) {
            policy.attachment_count = (policy.attachment_count - 1).max(0);
        }
        Ok(())
    }

    /// List attached role policies
    pub fn list_attached_role_policies(&self, role_name: &str) -> Result<Vec<String>, IamError> {
        let role = self
            .roles
            .get(role_name)
            .ok_or_else(|| IamError::NoSuchEntity(role_name.to_string()))?;
        Ok(role.attached_policies.clone())
    }
}

/// IAM errors
#[derive(Debug, thiserror::Error)]
pub enum IamError {
    #[error("Entity already exists: {0}")]
    EntityAlreadyExists(String),

    #[error("No such entity: {0}")]
    NoSuchEntity(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

/// State for IAM handlers
pub struct IamState {
    pub storage: Arc<IamStorage>,
}

impl IamState {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(IamStorage::new()),
        }
    }
}

impl Default for IamState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get_role() {
        let storage = IamStorage::new();

        let role = storage
            .create_role(
                "TestRole",
                r#"{"Version":"2012-10-17","Statement":[]}"#,
                Some("Test role".to_string()),
                None,
            )
            .unwrap();

        assert_eq!(role.role_name, "TestRole");
        assert!(role.arn.contains("TestRole"));

        let retrieved = storage.get_role("TestRole").unwrap();
        assert_eq!(retrieved.role_name, "TestRole");
    }

    #[test]
    fn test_attach_policy_to_role() {
        let storage = IamStorage::new();

        storage.create_role("TestRole", "{}", None, None).unwrap();

        let policy = storage
            .create_policy("TestPolicy", "{}", None, None)
            .unwrap();

        storage.attach_role_policy("TestRole", &policy.arn).unwrap();

        let attached = storage.list_attached_role_policies("TestRole").unwrap();
        assert!(attached.contains(&policy.arn));
    }

    #[test]
    fn test_duplicate_role_fails() {
        let storage = IamStorage::new();

        storage.create_role("TestRole", "{}", None, None).unwrap();

        let result = storage.create_role("TestRole", "{}", None, None);
        assert!(matches!(result, Err(IamError::EntityAlreadyExists(_))));
    }
}
