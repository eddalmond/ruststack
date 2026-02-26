//! HTTP handlers for CloudFormation

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

use crate::storage::{CloudFormationError, CloudFormationState};

pub async fn handle_request(
    State(state): State<Arc<CloudFormationState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let target = headers
        .get("x-amz-target")
        .and_then(|v: &HeaderValue| v.to_str().ok())
        .unwrap_or("");

    info!(target = %target, "CloudFormation request");

    match target {
        "AWSCloudFormation.CreateStack" => handle_create_stack(state, body).await,
        "AWSCloudFormation.DescribeStacks" => handle_describe_stacks(state, body).await,
        "AWSCloudFormation.DeleteStack" => handle_delete_stack(state, body).await,
        "AWSCloudFormation.UpdateStack" => handle_update_stack(state, body).await,
        "AWSCloudFormation.ListStacks" => handle_list_stacks(state, body).await,
        "AWSCloudFormation.ValidateTemplate" => handle_validate_template(state, body).await,
        "AWSCloudFormation.GetTemplate" => handle_get_template(state, body).await,
        "AWSCloudFormation.DescribeStackResources" => {
            handle_describe_stack_resources(state, body).await
        }
        _ => {
            warn!(target = %target, "Unknown CloudFormation operation");
            error_response(
                StatusCode::BAD_REQUEST,
                "UnknownOperationException",
                &format!("Unknown operation: {}", target),
            )
        }
    }
}

#[derive(Debug, Deserialize)]
struct CreateStackInput {
    #[serde(rename = "StackName")]
    stack_name: String,
    #[serde(rename = "TemplateBody")]
    template_body: Option<String>,
    #[serde(rename = "TemplateURL")]
    template_url: Option<String>,
    #[serde(rename = "Parameters")]
    parameters: Option<Vec<serde_json::Value>>,
}

async fn handle_create_stack(state: Arc<CloudFormationState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    let input: CreateStackInput = match serde_json::from_str(&body_str) {
        Ok(i) => i,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                &format!("Invalid JSON: {}", e),
            );
        }
    };

    let template_body = match input.template_body {
        Some(t) => t,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                "TemplateBody is required",
            );
        }
    };

    match state.create_stack(input.stack_name, template_body) {
        Ok(stack) => json_response(
            StatusCode::OK,
            &serde_json::json!({
                "StackId": stack.stack_id
            }),
        ),
        Err(CloudFormationError::StackAlreadyExists(name)) => error_response(
            StatusCode::BAD_REQUEST,
            "AlreadyExistsException",
            &format!("Stack [{}] already exists", name),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            &e.to_string(),
        ),
    }
}

#[derive(Debug, Deserialize)]
struct DescribeStacksInput {
    #[serde(rename = "StackName")]
    stack_name: Option<String>,
}

#[derive(Debug, Serialize)]
struct StackInfo {
    #[serde(rename = "StackId")]
    stack_id: String,
    #[serde(rename = "StackName")]
    stack_name: String,
    #[serde(rename = "CreationTime")]
    creation_time: String,
    #[serde(rename = "StackStatus")]
    stack_status: String,
    #[serde(rename = "Outputs")]
    outputs: Vec<serde_json::Value>,
}

async fn handle_describe_stacks(state: Arc<CloudFormationState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    let input: DescribeStacksInput = match serde_json::from_str(&body_str) {
        Ok(i) => i,
        Err(_) => DescribeStacksInput { stack_name: None },
    };

    match input.stack_name {
        Some(name) => match state.describe_stack(&name) {
            Ok(stack) => {
                let outputs = extract_outputs(&stack.outputs);
                json_response(
                    StatusCode::OK,
                    &serde_json::json!({
                        "Stacks": [{
                            "StackId": stack.stack_id,
                            "StackName": stack.stack_name,
                            "CreationTime": format!("{:.3}", stack.creation_time as f64),
                            "StackStatus": stack.status,
                            "Outputs": outputs
                        }]
                    }),
                )
            }
            Err(CloudFormationError::StackNotFound(_)) => {
                error_response(StatusCode::NOT_FOUND, "ValidationError", "Stack not found")
            }
            Err(e) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "InternalError",
                &e.to_string(),
            ),
        },
        None => {
            let stacks = state.list_stacks();
            let stack_infos: Vec<StackInfo> = stacks
                .into_iter()
                .map(|s| {
                    let outputs = extract_outputs(&s.outputs);
                    StackInfo {
                        stack_id: s.stack_id,
                        stack_name: s.stack_name,
                        creation_time: format!("{:.3}", s.creation_time as f64),
                        stack_status: s.status,
                        outputs,
                    }
                })
                .collect();

            json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "Stacks": stack_infos
                }),
            )
        }
    }
}

fn extract_outputs(template_outputs: &serde_json::Value) -> Vec<serde_json::Value> {
    if let Some(obj) = template_outputs.as_object() {
        if let Some(outputs) = obj.get("Outputs") {
            if let Some(arr) = outputs.as_array() {
                return arr.clone();
            }
            if let Some(outputs_obj) = outputs.as_object() {
                return outputs_obj
                    .iter()
                    .map(|(key, value)| {
                        serde_json::json!({
                            "OutputKey": key,
                            "OutputValue": value.get("value").or(Some(value)).unwrap_or(&serde_json::Value::Null),
                            "Description": value.get("description").unwrap_or(&serde_json::Value::Null)
                        })
                    })
                    .collect();
            }
        }
    }
    vec![]
}

#[derive(Debug, Deserialize)]
struct DeleteStackInput {
    #[serde(rename = "StackName")]
    stack_name: String,
}

async fn handle_delete_stack(state: Arc<CloudFormationState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    let input: DeleteStackInput = match serde_json::from_str(&body_str) {
        Ok(i) => i,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                "Missing StackName",
            );
        }
    };

    match state.delete_stack(&input.stack_name) {
        Ok(()) => json_response(StatusCode::OK, &serde_json::json!({})),
        Err(CloudFormationError::StackNotFound(_)) => {
            error_response(StatusCode::NOT_FOUND, "ValidationError", "Stack not found")
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            &e.to_string(),
        ),
    }
}

#[derive(Debug, Deserialize)]
struct UpdateStackInput {
    #[serde(rename = "StackName")]
    stack_name: String,
    #[serde(rename = "TemplateBody")]
    template_body: Option<String>,
}

async fn handle_update_stack(state: Arc<CloudFormationState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    let input: UpdateStackInput = match serde_json::from_str(&body_str) {
        Ok(i) => i,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                &format!("Invalid JSON: {}", e),
            );
        }
    };

    let template_body = match input.template_body {
        Some(t) => t,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                "TemplateBody is required",
            );
        }
    };

    match state.update_stack(&input.stack_name, template_body) {
        Ok(stack) => json_response(
            StatusCode::OK,
            &serde_json::json!({
                "StackId": stack.stack_id
            }),
        ),
        Err(CloudFormationError::StackNotFound(_)) => {
            error_response(StatusCode::NOT_FOUND, "ValidationError", "Stack not found")
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            &e.to_string(),
        ),
    }
}

async fn handle_list_stacks(state: Arc<CloudFormationState>, _body: Bytes) -> Response {
    let stacks = state.list_stacks();
    let stack_summaries: Vec<serde_json::Value> = stacks
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "StackId": s.stack_id,
                "StackName": s.stack_name,
                "CreationTime": format!("{:.3}", s.creation_time as f64),
                "StackStatus": s.status
            })
        })
        .collect();

    json_response(
        StatusCode::OK,
        &serde_json::json!({
            "StackSummaries": stack_summaries
        }),
    )
}

#[derive(Debug, Deserialize)]
struct ValidateTemplateInput {
    #[serde(rename = "TemplateBody")]
    template_body: Option<String>,
    #[serde(rename = "TemplateURL")]
    template_url: Option<String>,
}

async fn handle_validate_template(state: Arc<CloudFormationState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    let input: ValidateTemplateInput = match serde_json::from_str(&body_str) {
        Ok(i) => i,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                &format!("Invalid JSON: {}", e),
            );
        }
    };

    let template_body = match input.template_body {
        Some(t) => t,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                "TemplateBody is required",
            );
        }
    };

    match state.validate_template(&template_body) {
        Ok(result) => json_response(StatusCode::OK, &result),
        Err(e) => error_response(StatusCode::BAD_REQUEST, "ValidationError", &e.to_string()),
    }
}

#[derive(Debug, Deserialize)]
struct GetTemplateInput {
    #[serde(rename = "StackName")]
    stack_name: String,
}

async fn handle_get_template(state: Arc<CloudFormationState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    let input: GetTemplateInput = match serde_json::from_str(&body_str) {
        Ok(i) => i,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                "Missing StackName",
            );
        }
    };

    match state.describe_stack(&input.stack_name) {
        Ok(stack) => json_response(
            StatusCode::OK,
            &serde_json::json!({
                "TemplateBody": stack.template
            }),
        ),
        Err(CloudFormationError::StackNotFound(_)) => {
            error_response(StatusCode::NOT_FOUND, "ValidationError", "Stack not found")
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            &e.to_string(),
        ),
    }
}

#[derive(Debug, Deserialize)]
struct DescribeStackResourcesInput {
    #[serde(rename = "StackName")]
    stack_name: String,
}

async fn handle_describe_stack_resources(state: Arc<CloudFormationState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    let input: DescribeStackResourcesInput = match serde_json::from_str(&body_str) {
        Ok(i) => i,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "ValidationError",
                "Missing StackName",
            );
        }
    };

    match state.describe_stack(&input.stack_name) {
        Ok(stack) => {
            let resources: Vec<serde_json::Value> = stack
                .resources
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    serde_json::json!({
                        "LogicalResourceId": name,
                        "ResourceType": "AWS::CloudFormation::Stack",
                        "PhysicalResourceId": format!("{}-{}", stack.stack_name, i),
                        "ResourceStatus": "CREATE_COMPLETE",
                        "Timestamp": format!("{:.3}", stack.creation_time as f64)
                    })
                })
                .collect();

            json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "StackResources": resources
                }),
            )
        }
        Err(CloudFormationError::StackNotFound(_)) => {
            error_response(StatusCode::NOT_FOUND, "ValidationError", "Stack not found")
        }
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
