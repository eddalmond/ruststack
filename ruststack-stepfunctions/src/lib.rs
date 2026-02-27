//! AWS Step Functions Implementation
//!
//! Provides parsing for Amazon States Language (ASL) and state machine execution.

pub mod handlers;
pub mod storage;

pub use handlers::handle_request;
pub use storage::{StepFunctionsError, StepFunctionsState};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StateMachine {
    #[serde(rename = "StartAt")]
    pub start_at: String,
    #[serde(rename = "States")]
    pub states: HashMap<String, State>,
    #[serde(default, rename = "Comment")]
    pub comment: Option<String>,
    #[serde(default, rename = "Version")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "Type")]
pub enum State {
    #[serde(rename = "Pass")]
    Pass {
        #[serde(default)]
        result: Option<serde_json::Value>,
        #[serde(default, rename = "ResultPath")]
        result_path: Option<String>,
        #[serde(default, rename = "OutputPath")]
        output_path: Option<String>,
        #[serde(default)]
        end: bool,
        #[serde(default)]
        next: Option<String>,
    },

    #[serde(rename = "Task")]
    Task {
        #[serde(rename = "Resource")]
        resource: String,
        #[serde(default, rename = "ResultPath")]
        result_path: Option<String>,
        #[serde(default, rename = "OutputPath")]
        output_path: Option<String>,
        #[serde(default)]
        retry: Vec<Retry>,
        #[serde(default)]
        catch: Vec<Catcher>,
        #[serde(default)]
        end: bool,
        #[serde(default)]
        next: Option<String>,
    },

    #[serde(rename = "Choice")]
    Choice {
        #[serde(default, rename = "Choices")]
        choices: Vec<ChoiceRule>,
        #[serde(default)]
        default: Option<String>,
        #[serde(default)]
        end: bool,
    },

    #[serde(rename = "Wait")]
    Wait {
        #[serde(default, rename = "Seconds")]
        seconds: Option<u64>,
        #[serde(default, rename = "SecondsPath")]
        seconds_path: Option<String>,
        #[serde(default, rename = "Timestamp")]
        timestamp: Option<String>,
        #[serde(default, rename = "TimestampPath")]
        timestamp_path: Option<String>,
        #[serde(default)]
        end: bool,
        #[serde(default)]
        next: Option<String>,
    },

    #[serde(rename = "Succeed")]
    Succeed {
        #[serde(default)]
        output: Option<serde_json::Value>,
        #[serde(default, rename = "OutputPath")]
        output_path: Option<String>,
    },

    #[serde(rename = "Fail")]
    Fail {
        #[serde(rename = "Error")]
        error: String,
        #[serde(rename = "Cause")]
        cause: String,
    },

    #[serde(rename = "Parallel")]
    Parallel {
        #[serde(default, rename = "Branches")]
        branches: Vec<StateMachine>,
        #[serde(default, rename = "ResultPath")]
        result_path: Option<String>,
        #[serde(default, rename = "OutputPath")]
        output_path: Option<String>,
        #[serde(default)]
        retry: Vec<Retry>,
        #[serde(default)]
        catch: Vec<Catcher>,
        #[serde(default)]
        end: bool,
        #[serde(default)]
        next: Option<String>,
    },

    #[serde(rename = "Map")]
    Map {
        #[serde(default, rename = "Iterator")]
        iterator: Box<StateMachine>,
        #[serde(default, rename = "ItemsPath")]
        items_path: Option<String>,
        #[serde(default, rename = "ResultPath")]
        result_path: Option<String>,
        #[serde(default, rename = "OutputPath")]
        output_path: Option<String>,
        #[serde(default)]
        max_concurrency: Option<u64>,
        #[serde(default)]
        retry: Vec<Retry>,
        #[serde(default)]
        catch: Vec<Catcher>,
        #[serde(default)]
        end: bool,
        #[serde(default)]
        next: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChoiceRule {
    #[serde(default, rename = "Variable")]
    pub variable: Option<String>,
    #[serde(default, rename = "StringEquals")]
    pub string_equals: Option<String>,
    #[serde(default, rename = "StringEqualsPath")]
    pub string_equals_path: Option<String>,
    #[serde(default, rename = "StringLessThan")]
    pub string_less_than: Option<String>,
    #[serde(default, rename = "StringGreaterThan")]
    pub string_greater_than: Option<String>,
    #[serde(default, rename = "StringLessThanEquals")]
    pub string_less_than_equals: Option<String>,
    #[serde(default, rename = "StringGreaterThanEquals")]
    pub string_greater_than_equals: Option<String>,
    #[serde(default, rename = "NumericEquals")]
    pub numeric_equals: Option<f64>,
    #[serde(default, rename = "NumericEqualsPath")]
    pub numeric_equals_path: Option<String>,
    #[serde(default, rename = "NumericLessThan")]
    pub numeric_less_than: Option<f64>,
    #[serde(default, rename = "NumericGreaterThan")]
    pub numeric_greater_than: Option<f64>,
    #[serde(default, rename = "BooleanEquals")]
    pub boolean_equals: Option<bool>,
    #[serde(default, rename = "TimestampEquals")]
    pub timestamp_equals: Option<String>,
    #[serde(default, rename = "IsPresent")]
    pub is_present: Option<bool>,
    #[serde(default, rename = "IsNull")]
    pub is_null: Option<bool>,
    #[serde(default)]
    pub next: Option<String>,
    #[serde(default, rename = "And")]
    pub and: Vec<ChoiceRule>,
    #[serde(default, rename = "Or")]
    pub or: Vec<ChoiceRule>,
    #[serde(default, rename = "Not")]
    pub not: Option<Box<ChoiceRule>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Retry {
    #[serde(rename = "ErrorEquals")]
    pub error_equals: Vec<String>,
    #[serde(default, rename = "IntervalSeconds")]
    pub interval_seconds: Option<u64>,
    #[serde(default, rename = "MaxAttempts")]
    pub max_attempts: Option<u64>,
    #[serde(default, rename = "BackoffRate")]
    pub backoff_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catcher {
    #[serde(rename = "ErrorEquals")]
    pub error_equals: Vec<String>,
    #[serde(rename = "Next")]
    pub next: String,
    #[serde(default, rename = "ResultPath")]
    pub result_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMachineDefinition {
    pub arn: String,
    pub name: String,
    pub definition: StateMachine,
    pub role_arn: Option<String>,
    pub created_date: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Execution {
    pub arn: String,
    pub name: String,
    pub state_machine_arn: String,
    pub status: ExecutionStatus,
    pub input: serde_json::Value,
    pub output: Option<serde_json::Value>,
    pub start_date: chrono::DateTime<chrono::Utc>,
    pub stop_date: Option<chrono::DateTime<chrono::Utc>>,
    pub current_state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionStatus {
    #[serde(rename = "RUNNING")]
    Running,
    #[serde(rename = "SUCCEEDED")]
    Succeeded,
    #[serde(rename = "FAILED")]
    Failed,
    #[serde(rename = "ABORTED")]
    Aborted,
    #[serde(rename = "TIMED_OUT")]
    TimedOut,
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("Invalid state: {0}")]
    InvalidState(String),
    #[error("No matching choice: {0}")]
    NoMatchingChoice(String),
    #[error("Task failed: {0}")]
    TaskFailed(String, String),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

pub struct ExecutionContext {
    pub input: serde_json::Value,
    pub state_name: String,
    pub variables: HashMap<String, serde_json::Value>,
}

pub fn parse_state_machine(json: &str) -> Result<StateMachine, serde_json::Error> {
    serde_json::from_str(json)
}

pub fn get_next_state(state_machine: &StateMachine, current_state: &str) -> Option<String> {
    let state = state_machine.states.get(current_state)?;

    match state {
        State::Pass { next, end, .. } => {
            if *end {
                None
            } else {
                next.clone()
            }
        }
        State::Task { next, end, .. } => {
            if *end {
                None
            } else {
                next.clone()
            }
        }
        State::Choice { .. } => None,
        State::Wait { next, end, .. } => {
            if *end {
                None
            } else {
                next.clone()
            }
        }
        State::Succeed { .. } => None,
        State::Fail { .. } => None,
        State::Parallel { next, end, .. } => {
            if *end {
                None
            } else {
                next.clone()
            }
        }
        State::Map { next, end, .. } => {
            if *end {
                None
            } else {
                next.clone()
            }
        }
    }
}

pub fn evaluate_choice(choices: &[ChoiceRule], input: &serde_json::Value) -> Option<String> {
    for choice in choices {
        if let Some(next) = evaluate_single_choice(choice, input) {
            return Some(next);
        }
    }
    None
}

fn evaluate_single_choice(choice: &ChoiceRule, input: &serde_json::Value) -> Option<String> {
    if let Some(ref not_choice) = choice.not {
        if evaluate_single_choice(not_choice, input).is_some() {
            return None;
        }
        return choice.next.clone();
    }

    if !choice.and.is_empty() {
        let all_match = choice
            .and
            .iter()
            .all(|c| evaluate_single_choice(c, input).is_some());
        if all_match {
            return choice.next.clone();
        }
    }

    if !choice.or.is_empty() {
        let any_match = choice
            .or
            .iter()
            .any(|c| evaluate_single_choice(c, input).is_some());
        if any_match {
            return choice.next.clone();
        }
    }

    if let Some(var) = &choice.variable {
        let value = extract_path(input, &format!("$.{}", var.trim_start_matches("$.")));

        if let Ok(v) = value {
            if let Some(eq) = &choice.string_equals {
                if let serde_json::Value::String(s) = &v {
                    if s == eq {
                        return choice.next.clone();
                    }
                }
            }
            if let Some(eq) = &choice.numeric_equals {
                if let serde_json::Value::Number(n) = &v {
                    if let Some(f) = n.as_f64() {
                        if (f - eq).abs() < f64::EPSILON {
                            return choice.next.clone();
                        }
                    }
                }
            }
            if let Some(eq) = &choice.boolean_equals {
                if let serde_json::Value::Bool(b) = &v {
                    if b == eq {
                        return choice.next.clone();
                    }
                }
            }
        }
    }

    choice.next.clone()
}

pub fn apply_result_path(
    ctx: &mut ExecutionContext,
    result: serde_json::Value,
    result_path: Option<&str>,
) {
    match result_path {
        Some("$") => {
            ctx.input = result;
        }
        Some(path) => {
            if let Some(stripped) = path.strip_prefix("$.") {
                let path_to_use = format!("$.{}", stripped);
                if let Ok(_value) = extract_path(&ctx.input, &path_to_use) {
                    let mut new_input = ctx.input.clone();
                    if let serde_json::Value::Object(ref mut m) = new_input {
                        m.insert(stripped.to_string(), result);
                    } else {
                        ctx.input = result;
                    }
                    ctx.input = new_input;
                } else {
                    let mut new_input = serde_json::Map::new();
                    new_input.insert(stripped.to_string(), result);
                    ctx.input = serde_json::Value::Object(new_input);
                }
            }
        }
        None => {}
    }
}

pub fn extract_path(
    input: &serde_json::Value,
    path: &str,
) -> Result<serde_json::Value, ExecutionError> {
    if path == "$" || path.is_empty() {
        return Ok(input.clone());
    }

    if let Some(stripped) = path.strip_prefix("$.") {
        let parts: Vec<&str> = stripped.split('.').collect();
        let mut current = input.clone();

        for part in parts {
            match &current {
                serde_json::Value::Object(m) => {
                    current = m.get(part).cloned().unwrap_or(serde_json::Value::Null);
                }
                serde_json::Value::Array(arr) => {
                    if let Ok(idx) = part.parse::<usize>() {
                        current = arr.get(idx).cloned().unwrap_or(serde_json::Value::Null);
                    } else {
                        return Err(ExecutionError::InvalidState(format!(
                            "Invalid array index: {}",
                            part
                        )));
                    }
                }
                _ => {
                    return Ok(serde_json::Value::Null);
                }
            }
        }

        Ok(current)
    } else {
        Ok(input.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_state_machine() {
        let json = r#"
        {
            "StartAt": "PassState",
            "States": {
                "PassState": {
                    "Type": "Pass",
                    "Result": {"value": "hello"},
                    "End": true
                }
            }
        }
        "#;
        let sm = parse_state_machine(json).unwrap();
        assert_eq!(sm.start_at, "PassState");
        assert!(sm.states.contains_key("PassState"));
    }

    #[test]
    fn test_choice_evaluation() {
        let choices = vec![
            ChoiceRule {
                variable: Some("$.status".to_string()),
                string_equals: Some("active".to_string()),
                next: Some("ActiveState".to_string()),
                ..Default::default()
            },
            ChoiceRule {
                variable: Some("$.status".to_string()),
                string_equals: Some("inactive".to_string()),
                next: Some("InactiveState".to_string()),
                ..Default::default()
            },
        ];

        let input = serde_json::json!({"status": "active"});
        let result = evaluate_choice(&choices, &input);
        assert_eq!(result, Some("ActiveState".to_string()));
    }

    #[test]
    fn test_numeric_choice() {
        let choices = vec![ChoiceRule {
            variable: Some("$.count".to_string()),
            numeric_equals: Some(42.0),
            next: Some("AnswerState".to_string()),
            ..Default::default()
        }];

        let input = serde_json::json!({"count": 42});
        let result = evaluate_choice(&choices, &input);
        assert_eq!(result, Some("AnswerState".to_string()));
    }

    #[test]
    fn test_extract_path() {
        let input = serde_json::json!({
            "user": {
                "name": "Alice",
                "age": 30
            }
        });

        let result = extract_path(&input, "$.user.name").unwrap();
        assert_eq!(result, "Alice");
    }

    #[test]
    fn test_apply_result_path() {
        let mut ctx = ExecutionContext {
            input: serde_json::json!({"key": "value"}),
            state_name: "TestState".to_string(),
            variables: HashMap::new(),
        };

        apply_result_path(&mut ctx, serde_json::json!("new_value"), Some("$"));
        assert_eq!(ctx.input, "new_value");
    }

    #[test]
    fn test_parse_task_state() {
        let json = r#"
        {
            "StartAt": "MyTask",
            "States": {
                "MyTask": {
                    "Type": "Task",
                    "Resource": "arn:aws:lambda:us-east-1:123456789012:function:myFunction",
                    "End": true
                }
            }
        }
        "#;
        let sm = parse_state_machine(json).unwrap();
        assert_eq!(sm.start_at, "MyTask");
        assert!(sm.states.contains_key("MyTask"));
    }

    #[test]
    fn test_parse_choice_state() {
        let json = r#"
        {
            "StartAt": "CheckStatus",
            "States": {
                "CheckStatus": {
                    "Type": "Choice",
                    "Choices": [
                        {
                            "Variable": "$.status",
                            "StringEquals": "active",
                            "Next": "ActiveState"
                        }
                    ],
                    "Default": "UnknownState"
                },
                "ActiveState": {
                    "Type": "Pass",
                    "End": true
                },
                "UnknownState": {
                    "Type": "Pass",
                    "End": true
                }
            }
        }
        "#;
        let sm = parse_state_machine(json).unwrap();
        assert_eq!(sm.start_at, "CheckStatus");
    }

    #[test]
    fn test_parse_wait_state() {
        let json = r#"
        {
            "StartAt": "Wait",
            "States": {
                "Wait": {
                    "Type": "Wait",
                    "Seconds": 10,
                    "Next": "NextState"
                },
                "NextState": {
                    "Type": "Pass",
                    "End": true
                }
            }
        }
        "#;
        let sm = parse_state_machine(json).unwrap();
        assert_eq!(sm.start_at, "Wait");
        assert!(sm.states.contains_key("Wait"));
    }

    #[test]
    fn test_parse_parallel_state() {
        let json = r#"
        {
            "StartAt": "Parallel",
            "States": {
                "Parallel": {
                    "Type": "Parallel",
                    "Branches": [
                        {
                            "StartAt": "Branch1",
                            "States": {
                                "Branch1": {"Type": "Pass", "End": true}
                            }
                        }
                    ],
                    "End": true
                }
            }
        }
        "#;
        let sm = parse_state_machine(json).unwrap();
        assert!(sm.states.contains_key("Parallel"));
    }

    #[test]
    fn test_get_next_state() {
        let json = r#"
        {
            "StartAt": "Pass1",
            "States": {
                "Pass1": {
                    "Type": "Pass",
                    "Next": "Pass2"
                },
                "Pass2": {
                    "Type": "Pass",
                    "End": true
                }
            }
        }
        "#;
        let sm = parse_state_machine(json).unwrap();

        assert!(sm.states.contains_key("Pass1"));
        assert!(sm.states.contains_key("Pass2"));
    }

    #[test]
    fn test_boolean_choice() {
        let choices = vec![ChoiceRule {
            variable: Some("$.enabled".to_string()),
            boolean_equals: Some(true),
            next: Some("EnabledState".to_string()),
            ..Default::default()
        }];

        let input = serde_json::json!({"enabled": true});
        let result = evaluate_choice(&choices, &input);
        assert_eq!(result, Some("EnabledState".to_string()));
    }

    #[test]
    fn test_extract_array_path() {
        let input = serde_json::json!({
            "items": ["a", "b", "c"]
        });

        if let Ok(arr) = extract_path(&input, "$.items") {
            if let serde_json::Value::Array(a) = arr {
                assert_eq!(a.len(), 3);
            }
        }
    }

    #[test]
    fn test_extract_path_nested() {
        let input = serde_json::json!({
            "level1": {
                "level2": {
                    "value": "deep"
                }
            }
        });

        let result = extract_path(&input, "$.level1.level2.value");
        assert_eq!(result.unwrap(), "deep");
    }
}
