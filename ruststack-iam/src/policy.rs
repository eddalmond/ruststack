//! IAM Policy Evaluation Engine
//!
//! Provides deterministic policy evaluation for access control.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result of policy evaluation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    Allow,
    Deny,
    ImplicitDeny,
}

/// Evaluation context for a single API call
#[derive(Debug, Clone)]
pub struct EvaluationContext {
    /// Principal ARN (e.g., role ARN)
    pub principal_arn: Option<String>,
    /// Action being requested (e.g., "s3:GetObject")
    pub action: String,
    /// Resource ARN (e.g., "arn:aws:s3:::bucket/key")
    pub resource_arn: String,
    /// Service context (e.g., "s3", "dynamodb")
    pub service: String,
}

/// Policy evaluation engine
#[derive(Clone)]
pub struct PolicyEngine {
    enforce: bool,
}

impl PolicyEngine {
    pub fn new(enforce: bool) -> Self {
        Self { enforce }
    }

    /// Evaluate access based on policies attached to a role
    pub fn evaluate(&self, context: &EvaluationContext, policies: &[PolicyDocument]) -> Decision {
        if !self.enforce {
            return Decision::Allow;
        }

        let mut has_explicit_deny = false;
        let mut has_explicit_allow = false;

        for policy in policies {
            let decision = self.evaluate_policy_document(context, policy);
            match decision {
                Decision::Deny => has_explicit_deny = true,
                Decision::Allow => has_explicit_allow = true,
                Decision::ImplicitDeny => {}
            }
        }

        if has_explicit_deny {
            Decision::Deny
        } else if has_explicit_allow {
            Decision::Allow
        } else {
            Decision::ImplicitDeny
        }
    }

    fn evaluate_policy_document(
        &self,
        context: &EvaluationContext,
        policy: &PolicyDocument,
    ) -> Decision {
        let mut has_explicit_deny = false;
        let mut has_explicit_allow = false;

        for statement in &policy.statement {
            if !self.matches_principal(context, statement) {
                continue;
            }

            if !self.matches_action(context, &statement.action) {
                continue;
            }

            if !self.matches_resource(context, &statement.resource) {
                continue;
            }

            match statement.effect.as_str() {
                "Allow" => has_explicit_allow = true,
                "Deny" => has_explicit_deny = true,
                _ => {}
            }
        }

        if has_explicit_deny {
            Decision::Deny
        } else if has_explicit_allow {
            Decision::Allow
        } else {
            Decision::ImplicitDeny
        }
    }

    fn matches_principal(&self, context: &EvaluationContext, statement: &Statement) -> bool {
        // If no principal specified in policy, it applies to everyone (resource-based policies)
        if statement.principal.is_none() {
            return true;
        }

        let principal = statement.principal.as_ref().unwrap();

        // Handle AWS wildcard - principal: "*" means anyone
        if principal == "*" {
            return true;
        }

        // If we have a principal ARN from the context, check if it matches
        if let Some(arn) = &context.principal_arn {
            // Simple contains match - if policy says "role/test" and ARN contains that
            // This is a simplified check
            if principal.contains('*') {
                let pattern = principal.replace("*", ".*");
                if let Ok(re) = regex::Regex::new(&format!("^{}$", pattern)) {
                    return re.is_match(arn);
                }
            }
            // Direct contains check
            return arn.contains(principal);
        }

        // No principal in context but policy has principal - deny
        false
    }

    fn matches_action(&self, context: &EvaluationContext, action: &str) -> bool {
        // Handle wildcards in actions (e.g., "s3:*" matches "s3:GetObject")
        if action == "*" {
            return true;
        }

        // Handle "service:*" format
        if action.ends_with(":*") {
            let service_prefix = action.trim_end_matches(":*");
            return context.action.starts_with(service_prefix);
        }

        // Handle exact match or wildcards within action
        let context_action = &context.action;

        // Simple wildcard matching
        if action.contains('*') {
            let pattern = action.replace("*", ".*");
            if let Ok(re) = regex::Regex::new(&format!("^{}$", pattern)) {
                return re.is_match(context_action);
            }
        }

        action == context_action
    }

    fn matches_resource(&self, context: &EvaluationContext, resource: &str) -> bool {
        if resource == "*" {
            return true;
        }

        let res_arn = &context.resource_arn;

        // Handle wildcards in resources
        if resource.contains('*') {
            let pattern = resource.replace("*", ".*");
            if let Ok(re) = regex::Regex::new(&format!("^{}$", pattern)) {
                return re.is_match(res_arn);
            }
        }

        resource == res_arn
    }
}

/// AWS Policy Document structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PolicyDocument {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default, rename = "Statement")]
    pub statement: Vec<Statement>,
}

// Alias for compatibility
pub type Policy = PolicyDocument;

/// Statement within a policy
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Statement {
    #[serde(default)]
    pub sid: Option<String>,
    #[serde(rename = "Effect")]
    pub effect: String,
    #[serde(default)]
    pub principal: Option<String>,
    #[serde(rename = "Action", default)]
    pub action: String,
    #[serde(rename = "Resource", default)]
    pub resource: String,
    #[serde(default)]
    pub condition: Option<HashMap<String, HashMap<String, String>>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allow_wildcard() {
        let engine = PolicyEngine::new(true);

        let policy: PolicyDocument = serde_json::from_str(
            r#"{
            "Statement": [{
                "Effect": "Allow",
                "Action": "*",
                "Resource": "*"
            }]
        }"#,
        )
        .unwrap();

        let context = EvaluationContext {
            principal_arn: Some("arn:aws:iam::123456789012:role/test".to_string()),
            action: "s3:GetObject".to_string(),
            resource_arn: "arn:aws:s3:::bucket/key".to_string(),
            service: "s3".to_string(),
        };

        assert_eq!(engine.evaluate(&context, &[policy]), Decision::Allow);
    }

    #[test]
    fn test_explicit_deny_overrides_allow() {
        let engine = PolicyEngine::new(true);

        let allow_policy: PolicyDocument = serde_json::from_str(
            r#"{
            "Statement": [{
                "Effect": "Allow",
                "Action": "s3:GetObject",
                "Resource": "*"
            }]
        }"#,
        )
        .unwrap();

        let deny_policy: PolicyDocument = serde_json::from_str(
            r#"{
            "Statement": [{
                "Effect": "Deny",
                "Action": "s3:GetObject",
                "Resource": "arn:aws:s3:::secret-bucket/*"
            }]
        }"#,
        )
        .unwrap();

        let context = EvaluationContext {
            principal_arn: Some("arn:aws:iam::123456789012:role/test".to_string()),
            action: "s3:GetObject".to_string(),
            resource_arn: "arn:aws:s3:::secret-bucket/secret.txt".to_string(),
            service: "s3".to_string(),
        };

        assert_eq!(
            engine.evaluate(&context, &[allow_policy, deny_policy]),
            Decision::Deny
        );
    }

    #[test]
    fn test_no_enforcement_always_allows() {
        let engine = PolicyEngine::new(false);

        let context = EvaluationContext {
            principal_arn: None,
            action: "s3:GetObject".to_string(),
            resource_arn: "arn:aws:s3:::bucket/key".to_string(),
            service: "s3".to_string(),
        };

        assert_eq!(engine.evaluate(&context, &[]), Decision::Allow);
    }

    #[test]
    fn test_implicit_deny() {
        let engine = PolicyEngine::new(true);

        let policy: PolicyDocument = serde_json::from_str(
            r#"{
            "Statement": [{
                "Effect": "Allow",
                "Action": "s3:ListBucket",
                "Resource": "*"
            }]
        }"#,
        )
        .unwrap();

        let context = EvaluationContext {
            principal_arn: Some("arn:aws:iam::123456789012:role/test".to_string()),
            action: "s3:GetObject".to_string(),
            resource_arn: "arn:aws:s3:::bucket/key".to_string(),
            service: "s3".to_string(),
        };

        assert_eq!(engine.evaluate(&context, &[policy]), Decision::ImplicitDeny);
    }

    #[test]
    fn test_service_wildcard() {
        let engine = PolicyEngine::new(true);

        let policy: PolicyDocument = serde_json::from_str(
            r#"{
            "Statement": [{
                "Effect": "Allow",
                "Action": "s3:*",
                "Resource": "*"
            }]
        }"#,
        )
        .unwrap();

        let context = EvaluationContext {
            principal_arn: Some("arn:aws:iam::123456789012:role/test".to_string()),
            action: "s3:PutObject".to_string(),
            resource_arn: "arn:aws:s3:::bucket/key".to_string(),
            service: "s3".to_string(),
        };

        assert_eq!(engine.evaluate(&context, &[policy]), Decision::Allow);
    }
}
