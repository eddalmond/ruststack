//! CloudFormation in-memory storage

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::parse_json;
use crate::resolve_order;

#[derive(Error, Debug)]
pub enum CloudFormationError {
    #[error("Stack not found: {0}")]
    StackNotFound(String),
    #[error("Stack already exists: {0}")]
    StackAlreadyExists(String),
    #[error("Invalid template: {0}")]
    InvalidTemplate(String),
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Parse error: {0}")]
    ParseError(#[from] crate::ParseError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stack {
    pub stack_id: String,
    pub stack_name: String,
    pub template: String,
    pub resources: Vec<String>,
    pub outputs: serde_json::Value,
    pub status: String,
    pub creation_time: i64,
    pub last_updated_time: Option<i64>,
}

impl Stack {
    pub fn new(stack_name: String, template: String) -> Result<Self, CloudFormationError> {
        let now = chrono::Utc::now().timestamp();

        let parsed = parse_json(&template).or_else(|_| parse_yaml(&template))?;

        let resources = resolve_order(&parsed).unwrap_or_default();

        let outputs = serde_json::json!({
            "Outputs": parsed.outputs
        });

        let stack_id = format!(
            "arn:aws:cloudformation:us-east-1:000000000000:stack/{}/{}",
            stack_name, now
        );

        Ok(Self {
            stack_id,
            stack_name,
            template,
            resources,
            outputs,
            status: "CREATE_COMPLETE".to_string(),
            creation_time: now,
            last_updated_time: None,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct CloudFormationState {
    pub stacks: DashMap<String, Stack>,
}

impl CloudFormationState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_stack(
        &self,
        stack_name: String,
        template_body: String,
    ) -> Result<Stack, CloudFormationError> {
        if self.stacks.contains_key(&stack_name) {
            return Err(CloudFormationError::StackAlreadyExists(stack_name));
        }

        let stack = Stack::new(stack_name.clone(), template_body)?;
        self.stacks.insert(stack_name, stack.clone());
        Ok(stack)
    }

    pub fn describe_stack(&self, stack_name: &str) -> Result<Stack, CloudFormationError> {
        self.stacks
            .get(stack_name)
            .map(|s| s.clone())
            .ok_or_else(|| CloudFormationError::StackNotFound(stack_name.to_string()))
    }

    pub fn list_stacks(&self) -> Vec<Stack> {
        self.stacks.iter().map(|s| s.clone()).collect()
    }

    pub fn delete_stack(&self, stack_name: &str) -> Result<(), CloudFormationError> {
        if self.stacks.remove(stack_name).is_none() {
            return Err(CloudFormationError::StackNotFound(stack_name.to_string()));
        }
        Ok(())
    }

    pub fn update_stack(
        &self,
        stack_name: &str,
        template_body: String,
    ) -> Result<Stack, CloudFormationError> {
        let mut stack = self.describe_stack(stack_name)?;
        stack.template = template_body;
        stack.last_updated_time = Some(chrono::Utc::now().timestamp());
        stack.status = "UPDATE_COMPLETE".to_string();

        let parsed = parse_json(&stack.template).or_else(|_| parse_yaml(&stack.template))?;
        stack.resources = resolve_order(&parsed).unwrap_or_default();

        self.stacks.insert(stack_name.to_string(), stack.clone());
        Ok(stack)
    }

    pub fn validate_template(
        &self,
        template_body: &str,
    ) -> Result<serde_json::Value, CloudFormationError> {
        let parsed = parse_json(template_body).or_else(|_| parse_yaml(template_body))?;

        let capabilities: Vec<String> = vec![];
        let resources: Vec<String> = parsed.resources.keys().cloned().collect();

        Ok(serde_json::json!({
            "Capabilities": capabilities,
            "DeclaredDependencies": resources,
            "Description": parsed.description.unwrap_or_default(),
            "Parameters": []
        }))
    }
}

fn parse_yaml(yaml: &str) -> Result<crate::Template, CloudFormationError> {
    crate::parse_yaml(yaml).map_err(|e| CloudFormationError::InvalidTemplate(e.to_string()))
}
