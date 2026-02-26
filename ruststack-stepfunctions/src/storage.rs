//! StepFunctions in-memory storage

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StepFunctionsError {
    #[error("State machine not found: {0}")]
    StateMachineNotFound(String),
    #[error("State machine already exists: {0}")]
    StateMachineAlreadyExists(String),
    #[error("Execution not found: {0}")]
    ExecutionNotFound(String),
    #[error("Invalid state: {0}")]
    InvalidState(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMachineInfo {
    pub arn: String,
    pub name: String,
    pub definition: String,
    pub role_arn: Option<String>,
    pub created_date: i64,
    pub state_machine_type: String,
}

impl StateMachineInfo {
    pub fn new(name: String, definition: String, role_arn: Option<String>) -> Self {
        let now = chrono::Utc::now().timestamp();
        let arn = format!(
            "arn:aws:states:us-east-1:000000000000:stateMachine:{}",
            name
        );
        Self {
            arn,
            name,
            definition,
            role_arn,
            created_date: now,
            state_machine_type: "STANDARD".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionInfo {
    pub arn: String,
    pub name: String,
    pub state_machine_arn: String,
    pub status: String,
    pub input: serde_json::Value,
    pub output: Option<serde_json::Value>,
    pub start_date: i64,
    pub stop_date: Option<i64>,
    pub current_state: Option<String>,
    pub definition: Option<String>,
}

impl ExecutionInfo {
    pub fn new(
        name: String,
        state_machine_arn: String,
        input: serde_json::Value,
        definition: String,
    ) -> Self {
        let now = chrono::Utc::now().timestamp();
        let arn = format!(
            "arn:aws:states:us-east-1:000000000000:execution:{}:{}",
            state_machine_arn.split(':').next_back().unwrap_or("Unknown"),
            name
        );
        Self {
            arn,
            name,
            state_machine_arn,
            status: "RUNNING".to_string(),
            input,
            output: None,
            start_date: now,
            stop_date: None,
            current_state: None,
            definition: Some(definition),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct StepFunctionsState {
    pub state_machines: DashMap<String, StateMachineInfo>,
    pub executions: DashMap<String, ExecutionInfo>,
}

impl StepFunctionsState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_state_machine(
        &self,
        name: String,
        definition: String,
        role_arn: Option<String>,
    ) -> Result<StateMachineInfo, StepFunctionsError> {
        if self.state_machines.contains_key(&name) {
            return Err(StepFunctionsError::StateMachineAlreadyExists(name));
        }

        let sm = StateMachineInfo::new(name.clone(), definition, role_arn);
        self.state_machines.insert(name, sm.clone());
        Ok(sm)
    }

    pub fn describe_state_machine(
        &self,
        name: &str,
    ) -> Result<StateMachineInfo, StepFunctionsError> {
        self.state_machines
            .get(name)
            .map(|sm| sm.clone())
            .ok_or_else(|| StepFunctionsError::StateMachineNotFound(name.to_string()))
    }

    pub fn list_state_machines(&self) -> Vec<StateMachineInfo> {
        self.state_machines.iter().map(|sm| sm.clone()).collect()
    }

    pub fn delete_state_machine(&self, name: &str) -> Result<(), StepFunctionsError> {
        if self.state_machines.remove(name).is_none() {
            return Err(StepFunctionsError::StateMachineNotFound(name.to_string()));
        }
        Ok(())
    }

    pub fn start_execution(
        &self,
        name: String,
        state_machine_name: &str,
        input: serde_json::Value,
    ) -> Result<ExecutionInfo, StepFunctionsError> {
        let sm = self.describe_state_machine(state_machine_name)?;

        let execution = ExecutionInfo::new(name, sm.arn.clone(), input, sm.definition.clone());

        self.executions
            .insert(execution.arn.clone(), execution.clone());
        Ok(execution)
    }

    pub fn describe_execution(
        &self,
        execution_arn: &str,
    ) -> Result<ExecutionInfo, StepFunctionsError> {
        self.executions
            .get(execution_arn)
            .map(|e| e.clone())
            .ok_or_else(|| StepFunctionsError::ExecutionNotFound(execution_arn.to_string()))
    }

    pub fn list_executions(&self, state_machine_arn: &str) -> Vec<ExecutionInfo> {
        self.executions
            .iter()
            .filter(|e| e.state_machine_arn == state_machine_arn)
            .map(|e| e.clone())
            .collect()
    }

    pub fn update_execution(&self, execution: ExecutionInfo) {
        self.executions.insert(execution.arn.clone(), execution);
    }
}
