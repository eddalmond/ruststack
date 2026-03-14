//! HTTP handlers for Step Functions

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::Response,
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};

use crate::storage::{StepFunctionsError, StepFunctionsState};

#[derive(Debug, Serialize)]
struct CreateStateMachineResponse {
    #[serde(rename = "stateMachineArn")]
    state_machine_arn: String,
    #[serde(rename = "creationDate")]
    creation_date: String,
}

#[derive(Debug, Serialize)]
struct DescribeStateMachineResponse {
    #[serde(rename = "stateMachineArn")]
    state_machine_arn: String,
    name: String,
    definition: String,
    #[serde(rename = "roleArn")]
    role_arn: Option<String>,
    #[serde(rename = "type")]
    machine_type: String,
    #[serde(rename = "creationDate")]
    creation_date: String,
}

#[derive(Debug, Serialize)]
struct StartExecutionResponse {
    #[serde(rename = "executionArn")]
    execution_arn: String,
    #[serde(rename = "startDate")]
    start_date: String,
    status: String,
}

#[derive(Debug, Serialize)]
struct DescribeExecutionResponse {
    #[serde(rename = "executionArn")]
    execution_arn: String,
    #[serde(rename = "stateMachineArn")]
    state_machine_arn: String,
    name: String,
    status: String,
    input: String,
    #[serde(rename = "output", skip_serializing_if = "Option::is_none")]
    output: Option<String>,
    #[serde(rename = "startDate")]
    start_date: String,
    #[serde(rename = "stopDate")]
    stop_date: Option<String>,
}

pub async fn handle_request(
    State(state): State<Arc<StepFunctionsState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let target = headers
        .get("x-amz-target")
        .and_then(|v: &HeaderValue| v.to_str().ok())
        .unwrap_or("");

    info!(target = %target, "StepFunctions request");

    match target {
        "AWSStepFunctions.CreateStateMachine" => handle_create_state_machine(state, body).await,
        "AWSStepFunctions.DescribeStateMachine" => handle_describe_state_machine(state, body).await,
        "AWSStepFunctions.DeleteStateMachine" => handle_delete_state_machine(state, body).await,
        "AWSStepFunctions.ListStateMachines" => handle_list_state_machines(state, body).await,
        "AWSStepFunctions.StartExecution" => handle_start_execution(state, body).await,
        "AWSStepFunctions.DescribeExecution" => handle_describe_execution(state, body).await,
        "AWSStepFunctions.ListExecutions" => handle_list_executions(state, body).await,
        "AWSStepFunctions.StopExecution" => handle_stop_execution(state, body).await,
        _ => {
            warn!(target = %target, "Unknown StepFunctions operation");
            error_response(
                StatusCode::BAD_REQUEST,
                "UnknownOperationException",
                &format!("Unknown operation: {}", target),
            )
        }
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CreateStateMachineInput {
    name: String,
    definition: String,
    #[serde(rename = "roleArn")]
    role_arn: Option<String>,
    #[serde(rename = "type")]
    machine_type: Option<String>,
}

async fn handle_create_state_machine(state: Arc<StepFunctionsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    let input: CreateStateMachineInput = match serde_json::from_str(&body_str) {
        Ok(i) => i,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "InvalidDefinition",
                &format!("Invalid JSON: {}", e),
            );
        }
    };

    match state.create_state_machine(input.name, input.definition, input.role_arn) {
        Ok(sm) => {
            let response = CreateStateMachineResponse {
                state_machine_arn: sm.arn,
                creation_date: format!("{:.3}", sm.created_date as f64),
            };
            json_response(StatusCode::OK, &response)
        }
        Err(StepFunctionsError::StateMachineAlreadyExists(name)) => error_response(
            StatusCode::BAD_REQUEST,
            "StateMachineAlreadyExists",
            &format!("State machine already exists: {}", name),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            &e.to_string(),
        ),
    }
}

async fn handle_describe_state_machine(state: Arc<StepFunctionsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    #[derive(Deserialize)]
    struct Input {
        #[serde(rename = "stateMachineArn")]
        state_machine_arn: String,
    }

    let input: Input = match serde_json::from_str(&body_str) {
        Ok(i) => i,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "InvalidInput",
                "Missing stateMachineArn",
            );
        }
    };

    let name = input.state_machine_arn.split(':').next_back().unwrap_or("");

    match state.describe_state_machine(name) {
        Ok(sm) => {
            let response = DescribeStateMachineResponse {
                state_machine_arn: sm.arn,
                name: sm.name,
                definition: sm.definition,
                role_arn: sm.role_arn,
                machine_type: sm.state_machine_type,
                creation_date: format!("{:.3}", sm.created_date as f64),
            };
            json_response(StatusCode::OK, &response)
        }
        Err(StepFunctionsError::StateMachineNotFound(_)) => error_response(
            StatusCode::NOT_FOUND,
            "StateMachineNotFound",
            "State machine not found",
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            &e.to_string(),
        ),
    }
}

async fn handle_delete_state_machine(state: Arc<StepFunctionsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    #[derive(Deserialize)]
    struct Input {
        #[serde(rename = "stateMachineArn")]
        state_machine_arn: String,
    }

    let input: Input = match serde_json::from_str(&body_str) {
        Ok(i) => i,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "InvalidInput",
                "Missing stateMachineArn",
            );
        }
    };

    let name = input.state_machine_arn.split(':').next_back().unwrap_or("");

    match state.delete_state_machine(name) {
        Ok(()) => json_response(StatusCode::OK, &serde_json::json!({})),
        Err(StepFunctionsError::StateMachineNotFound(_)) => error_response(
            StatusCode::NOT_FOUND,
            "StateMachineNotFound",
            "State machine not found",
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            &e.to_string(),
        ),
    }
}

#[derive(Debug, Serialize)]
struct StateMachineListItem {
    #[serde(rename = "stateMachineArn")]
    state_machine_arn: String,
    name: String,
    #[serde(rename = "type")]
    machine_type: String,
    #[serde(rename = "creationDate")]
    creation_date: String,
}

async fn handle_list_state_machines(state: Arc<StepFunctionsState>, _body: Bytes) -> Response {
    let machines = state.list_state_machines();

    let items: Vec<StateMachineListItem> = machines
        .into_iter()
        .map(|sm| StateMachineListItem {
            state_machine_arn: sm.arn,
            name: sm.name,
            machine_type: sm.state_machine_type,
            creation_date: format!("{:.3}", sm.created_date as f64),
        })
        .collect();

    json_response(
        StatusCode::OK,
        &serde_json::json!({
            "stateMachines": items
        }),
    )
}

#[derive(Debug, Deserialize)]
struct StartExecutionInput {
    #[serde(rename = "stateMachineArn")]
    state_machine_arn: String,
    name: Option<String>,
    input: Option<String>,
}

async fn handle_start_execution(state: Arc<StepFunctionsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    let input: StartExecutionInput = match serde_json::from_str(&body_str) {
        Ok(i) => i,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "InvalidInput",
                "Missing required fields",
            );
        }
    };

    let state_machine_name = input.state_machine_arn.split(':').next_back().unwrap_or("");

    let name = input
        .name
        .unwrap_or_else(|| format!("{}-{}", state_machine_name, chrono::Utc::now().timestamp()));

    let input_json: serde_json::Value = input
        .input
        .map(|s| serde_json::from_str(&s).unwrap_or(serde_json::json!({})))
        .unwrap_or(serde_json::json!({}));

    match state.start_execution(name, state_machine_name, input_json) {
        Ok(exec) => {
            let response = StartExecutionResponse {
                execution_arn: exec.arn,
                start_date: format!("{:.3}", exec.start_date as f64),
                status: exec.status,
            };
            json_response(StatusCode::OK, &response)
        }
        Err(StepFunctionsError::StateMachineNotFound(_)) => error_response(
            StatusCode::NOT_FOUND,
            "StateMachineNotFound",
            "State machine not found",
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            &e.to_string(),
        ),
    }
}

async fn handle_describe_execution(state: Arc<StepFunctionsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    #[derive(Deserialize)]
    struct Input {
        #[serde(rename = "executionArn")]
        execution_arn: String,
    }

    let input: Input = match serde_json::from_str(&body_str) {
        Ok(i) => i,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "InvalidInput",
                "Missing executionArn",
            );
        }
    };

    match state.describe_execution(&input.execution_arn) {
        Ok(exec) => {
            let response = DescribeExecutionResponse {
                execution_arn: exec.arn,
                state_machine_arn: exec.state_machine_arn,
                name: exec.name,
                status: exec.status,
                input: serde_json::to_string(&exec.input).unwrap_or_default(),
                output: exec
                    .output
                    .as_ref()
                    .map(|o| serde_json::to_string(o).unwrap_or_default()),
                start_date: format!("{:.3}", exec.start_date as f64),
                stop_date: exec.stop_date.map(|d| format!("{:.3}", d as f64)),
            };
            json_response(StatusCode::OK, &response)
        }
        Err(StepFunctionsError::ExecutionNotFound(_)) => error_response(
            StatusCode::NOT_FOUND,
            "ExecutionNotFound",
            "Execution not found",
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            &e.to_string(),
        ),
    }
}

#[derive(Debug, Serialize)]
struct ExecutionListItem {
    #[serde(rename = "executionArn")]
    execution_arn: String,
    name: String,
    #[serde(rename = "stateMachineArn")]
    state_machine_arn: String,
    status: String,
    #[serde(rename = "startDate")]
    start_date: String,
}

async fn handle_list_executions(state: Arc<StepFunctionsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    #[derive(Deserialize)]
    struct Input {
        #[serde(rename = "stateMachineArn")]
        state_machine_arn: String,
    }

    let input: Input = match serde_json::from_str(&body_str) {
        Ok(i) => i,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "InvalidInput",
                "Missing stateMachineArn",
            );
        }
    };

    let executions = state.list_executions(&input.state_machine_arn);

    let items: Vec<ExecutionListItem> = executions
        .into_iter()
        .map(|e| ExecutionListItem {
            execution_arn: e.arn,
            name: e.name,
            state_machine_arn: e.state_machine_arn,
            status: e.status,
            start_date: format!("{:.3}", e.start_date as f64),
        })
        .collect();

    json_response(
        StatusCode::OK,
        &serde_json::json!({
            "executions": items
        }),
    )
}

async fn handle_stop_execution(state: Arc<StepFunctionsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    #[derive(Deserialize)]
    struct Input {
        #[serde(rename = "executionArn")]
        execution_arn: String,
    }

    let input: Input = match serde_json::from_str(&body_str) {
        Ok(i) => i,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "InvalidInput",
                "Missing executionArn",
            );
        }
    };

    match state.describe_execution(&input.execution_arn) {
        Ok(mut exec) => {
            exec.status = "ABORTED".to_string();
            exec.stop_date = Some(chrono::Utc::now().timestamp());
            state.update_execution(exec);
            json_response(StatusCode::OK, &serde_json::json!({}))
        }
        Err(StepFunctionsError::ExecutionNotFound(_)) => error_response(
            StatusCode::NOT_FOUND,
            "ExecutionNotFound",
            "Execution not found",
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            &e.to_string(),
        ),
    }
}

fn json_response<T: Serialize>(status: StatusCode, value: &T) -> Response {
    let body = serde_json::to_string(value).unwrap_or_default();
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap()
}

fn error_response(status: StatusCode, error_type: &str, message: &str) -> Response {
    let body = serde_json::json!({
        "__type": error_type,
        "message": message
    });
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}
