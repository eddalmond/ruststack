//! IAM Policy Definitions and Evaluation Engine

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An IAM Policy Document
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolicyDocument {
    #[serde(rename = "Version")]
    pub version: Option<String>,
    #[serde(rename = "Statement")]
    pub statements: Vec<Statement>,
}

impl PolicyDocument {
    /// Creates a new policy document from a JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// A policy statement
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Statement {
    #[serde(rename = "Sid", skip_serializing_if = "Option::is_none")]
    pub sid: Option<String>,
    #[serde(rename = "Effect")]
    pub effect: Effect,
    #[serde(rename = "Principal", skip_serializing_if = "Option::is_none")]
    pub principal: Option<Principal>,
    #[serde(rename = "Action", deserialize_with = "deserialize_string_or_vec")]
    pub action: Vec<String>,
    #[serde(rename = "Resource", deserialize_with = "deserialize_string_or_vec")]
    pub resource: Vec<String>,
    #[serde(rename = "Condition", skip_serializing_if = "Option::is_none")]
    pub condition: Option<HashMap<String, HashMap<String, Vec<String>>>>,
}

/// Deserializes either a string or a sequence of strings into a Vec<String>
fn deserialize_string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct StringOrVec;

    impl<'de> serde::de::Visitor<'de> for StringOrVec {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("string or list of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(vec![value.to_owned()])
        }

        fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
        where
            S: serde::de::SeqAccess<'de>,
        {
            let mut vec = Vec::new();
            while let Some(value) = seq.next_element()? {
                vec.push(value);
            }
            Ok(vec)
        }
    }

    deserializer.deserialize_any(StringOrVec)
}

/// IAM effect (Allow or Deny)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Effect {
    #[serde(rename = "Allow")]
    Allow,
    #[serde(rename = "Deny")]
    Deny,
}

/// Principal definition
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Principal {
    All(String), // Matches "*"
    AWS(serde_json::Value),
    Service(serde_json::Value),
    Federated(serde_json::Value),
}

/// Decision from a policy evaluation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Deny,
    ImplicitDeny,
}

/// Context for policy evaluation
pub struct EvaluationContext<'a> {
    pub action: &'a str,
    pub resource: &'a str,
    pub principal_arn: Option<&'a str>,
    pub conditions: &'a HashMap<String, String>,
}

/// The policy evaluation engine
pub struct PolicyEngine {}

impl PolicyEngine {
    /// Evaluates multiple policies returning the final decision.
    /// Explicit Deny > Explicit Allow > Implicit Deny.
    pub fn evaluate(policies: &[PolicyDocument], context: &EvaluationContext) -> Decision {
        let mut has_allow = false;

        for policy in policies {
            match Self::evaluate_single(policy, context) {
                Decision::Deny => return Decision::Deny, // Early return on explicit deny
                Decision::Allow => has_allow = true,
                Decision::ImplicitDeny => {}
            }
        }

        if has_allow {
            Decision::Allow
        } else {
            Decision::ImplicitDeny
        }
    }

    fn evaluate_single(policy: &PolicyDocument, context: &EvaluationContext) -> Decision {
        let mut has_allow = false;

        for statement in &policy.statements {
            // Check if statement matches the action and resource
            if !Self::matches_action(&statement.action, context.action) {
                continue;
            }
            if !Self::matches_resource(&statement.resource, context.resource) {
                continue;
            }

            // Note: Conditions are not fully implemented, returning true for now
            // if let Some(ref conditions) = statement.condition { ... }

            match statement.effect {
                Effect::Deny => return Decision::Deny,
                Effect::Allow => has_allow = true,
            }
        }

        if has_allow {
            Decision::Allow
        } else {
            Decision::ImplicitDeny
        }
    }

    fn matches_action(actions: &[String], requested: &str) -> bool {
        for action in actions {
            if action == "*" {
                return true;
            }
            // Support trailing wildcard e.g., s3:Get*
            if action.ends_with('*') {
                let prefix = &action[..action.len() - 1];
                if requested.starts_with(prefix) {
                    return true;
                }
            }
            if action.eq_ignore_ascii_case(requested) {
                return true;
            }
        }
        false
    }

    fn matches_resource(resources: &[String], requested: &str) -> bool {
        for resource in resources {
            if resource == "*" {
                return true;
            }
            if resource.contains('*') || resource.contains('?') {
                if Self::matches_glob(resource, requested) {
                    return true;
                }
            }
            if resource == requested {
                return true;
            }
        }
        false
    }

    fn matches_glob(pattern: &str, target: &str) -> bool {
        // Minimal glob matching for ARN wildcards (* and ?)
        let mut p_indices = pattern.char_indices().peekable();
        let mut t_indices = target.char_indices().peekable();

        let mut next_p = p_indices.next();
        let mut next_t = t_indices.next();

        let mut fallback_p: Option<(usize, char)> = None;
        let mut fallback_t: Option<(usize, char)> = None;

        while let Some((_, p_char)) = next_p {
            if p_char == '*' {
                fallback_p = p_indices.peek().copied();
                fallback_t = next_t;
                next_p = p_indices.next();
                continue;
            }

            if let Some((_, t_char)) = next_t {
                if p_char == '?' || p_char == t_char {
                    next_p = p_indices.next();
                    next_t = t_indices.next();
                    continue;
                }
            }

            if let (Some(_), Some(ft)) = (fallback_p, fallback_t) {
                next_p = fallback_p;
                let mut temp_t = target[ft.0..].char_indices();
                temp_t.next(); // Consume the character we matched on '*'
                fallback_t = temp_t.next().map(|(i, c)| (ft.0 + i, c));
                next_t = fallback_t;
                continue;
            }

            return false;
        }

        next_t.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_deserialization() {
        let json = r#"{
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Effect": "Allow",
                    "Action": "s3:ListBucket",
                    "Resource": "arn:aws:s3:::example_bucket"
                },
                {
                    "Effect": "Allow",
                    "Action": ["s3:GetObject", "s3:PutObject"],
                    "Resource": ["arn:aws:s3:::example_bucket/*"]
                }
            ]
        }"#;

        let doc = PolicyDocument::from_json(json).unwrap();
        assert_eq!(doc.statements.len(), 2);
        assert_eq!(doc.statements[0].action, vec!["s3:ListBucket"]);
    }

    #[test]
    fn test_evaluate_allow() {
        let json = r#"{
            "Statement": [
                {
                    "Effect": "Allow",
                    "Action": ["s3:GetObject"],
                    "Resource": ["arn:aws:s3:::example_bucket/*"]
                }
            ]
        }"#;

        let doc = PolicyDocument::from_json(json).unwrap();
        let empty_conditions = HashMap::new();
        let ctx = EvaluationContext {
            action: "s3:GetObject",
            resource: "arn:aws:s3:::example_bucket/test.txt",
            principal_arn: None,
            conditions: &empty_conditions,
        };

        assert_eq!(PolicyEngine::evaluate(&[doc], &ctx), Decision::Allow);
    }

    #[test]
    fn test_evaluate_implicit_deny() {
        let json = r#"{
            "Statement": [
                {
                    "Effect": "Allow",
                    "Action": ["s3:GetObject"],
                    "Resource": ["arn:aws:s3:::example_bucket/*"]
                }
            ]
        }"#;

        let doc = PolicyDocument::from_json(json).unwrap();
        let empty_conditions = HashMap::new();
        // Requesting PutObject instead of GetObject
        let ctx = EvaluationContext {
            action: "s3:PutObject",
            resource: "arn:aws:s3:::example_bucket/test.txt",
            principal_arn: None,
            conditions: &empty_conditions,
        };

        assert_eq!(PolicyEngine::evaluate(&[doc], &ctx), Decision::ImplicitDeny);
    }

    #[test]
    fn test_evaluate_explicit_deny() {
        let json = r#"{
            "Statement": [
                {
                    "Effect": "Allow",
                    "Action": ["s3:*"],
                    "Resource": ["arn:aws:s3:::example_bucket/*"]
                },
                {
                    "Effect": "Deny",
                    "Action": ["s3:DeleteObject"],
                    "Resource": ["arn:aws:s3:::example_bucket/protected/*"]
                }
            ]
        }"#;

        let doc = PolicyDocument::from_json(json).unwrap();
        let empty_conditions = HashMap::new();

        // Should be allowed by the first rule
        let ctx1 = EvaluationContext {
            action: "s3:GetObject",
            resource: "arn:aws:s3:::example_bucket/protected/test.txt",
            principal_arn: None,
            conditions: &empty_conditions,
        };
        assert_eq!(PolicyEngine::evaluate(&[doc.clone()], &ctx1), Decision::Allow);

        // Should be denied by the second rule despite the first rule
        let ctx2 = EvaluationContext {
            action: "s3:DeleteObject",
            resource: "arn:aws:s3:::example_bucket/protected/test.txt",
            principal_arn: None,
            conditions: &empty_conditions,
        };
        assert_eq!(PolicyEngine::evaluate(&[doc], &ctx2), Decision::Deny);
    }

    #[test]
    fn test_glob_matching() {
        assert!(PolicyEngine::matches_glob("s3:*", "s3:GetObject"));
        assert!(PolicyEngine::matches_glob("arn:aws:s3:::bucket/*", "arn:aws:s3:::bucket/path/to/key"));
        assert!(!PolicyEngine::matches_glob("arn:aws:s3:::bucket/*", "arn:aws:sns:region:account:topic"));
        assert!(PolicyEngine::matches_glob("a*bc", "aaabbbc"));
        assert!(PolicyEngine::matches_glob("?at", "cat"));
        assert!(!PolicyEngine::matches_glob("?at", "chat"));
    }
}
