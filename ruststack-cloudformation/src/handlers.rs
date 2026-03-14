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
    let mut target = headers
        .get("x-amz-target")
        .and_then(|v: &HeaderValue| v.to_str().ok())
        .map(|s| s.replace("AWSCloudFormation.", ""))
        .unwrap_or_else(|| String::new());

    let body_str = String::from_utf8_lossy(&body);

    if target.is_empty() {
        for pair in body_str.split('&') {
            if let Some((key, val)) = pair.split_once('=') {
                if key == "Action" {
                    target = val.to_string();
                    break;
                }
            }
        }
    }

    info!(target = %target, "CloudFormation request");

    match target.as_str() {
        "CreateStack" => handle_create_stack(state, body).await,
        "DescribeStacks" => handle_describe_stacks(state, body).await,
        "DeleteStack" => handle_delete_stack(state, body).await,
        "UpdateStack" => handle_update_stack(state, body).await,
        "ListStacks" => handle_list_stacks(state, body).await,
        "ValidateTemplate" => handle_validate_template(state, body).await,
        "GetTemplate" => handle_get_template(state, body).await,
        "DescribeStackResources" => handle_describe_stack_resources(state, body).await,
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

fn parse_input<'a, T: serde::Deserialize<'a>>(body: &'a str) -> Result<T, String> {
    if body.trim_start().starts_with('{') {
        serde_json::from_str(body).map_err(|e| e.to_string())
    } else {
        serde_urlencoded::from_str(body).map_err(|e| e.to_string())
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CreateStackInput {
    #[serde(rename = "StackName")]
    stack_name: String,
    #[serde(rename = "TemplateBody")]
    template_body: Option<String>,
    #[serde(rename = "TemplateURL")]
    template_url: Option<String>,
}

async fn handle_create_stack(state: Arc<CloudFormationState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    let input: CreateStackInput = match parse_input(&body_str) {
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
        Ok(stack) => {
            let body = format!(
                r#"<CreateStackResponse xmlns="http://cloudformation.amazonaws.com/doc/2010-05-15/">
  <CreateStackResult>
    <StackId>{}</StackId>
  </CreateStackResult>
  <ResponseMetadata>
    <RequestId>00000000-0000-0000-0000-000000000000</RequestId>
  </ResponseMetadata>
</CreateStackResponse>"#,
                stack.stack_id
            );
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/xml")
                .body(Body::from(body))
                .unwrap()
        }
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

    let input: DescribeStacksInput = match parse_input(&body_str) {
        Ok(i) => i,
        Err(_) => DescribeStacksInput { stack_name: None },
    };

    match input.stack_name {
        Some(name) => match state.describe_stack(&name) {
            Ok(stack) => {
                // A very simplified XML response for DescribeStacks
                let body = format!(
                    r#"<DescribeStacksResponse xmlns="http://cloudformation.amazonaws.com/doc/2010-05-15/">
  <DescribeStacksResult>
    <Stacks>
      <member>
        <StackId>{}</StackId>
        <StackName>{}</StackName>
        <CreationTime>{}</CreationTime>
        <StackStatus>{}</StackStatus>
      </member>
    </Stacks>
  </DescribeStacksResult>
  <ResponseMetadata>
    <RequestId>00000000-0000-0000-0000-000000000000</RequestId>
  </ResponseMetadata>
</DescribeStacksResponse>"#,
                    stack.stack_id,
                    stack.stack_name,
                    format!("{:.3}", stack.creation_time as f64),
                    stack.status
                );
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/xml")
                    .body(Body::from(body))
                    .unwrap()
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
            let mut members = String::new();
            for s in stacks {
                members.push_str(&format!(
                    r#"      <member>
        <StackId>{}</StackId>
        <StackName>{}</StackName>
        <CreationTime>{}</CreationTime>
        <StackStatus>{}</StackStatus>
      </member>
"#,
                    s.stack_id,
                    s.stack_name,
                    format!("{:.3}", s.creation_time as f64),
                    s.status
                ));
            }

            let body = format!(
                r#"<DescribeStacksResponse xmlns="http://cloudformation.amazonaws.com/doc/2010-05-15/">
  <DescribeStacksResult>
    <Stacks>
{}    </Stacks>
  </DescribeStacksResult>
  <ResponseMetadata>
    <RequestId>00000000-0000-0000-0000-000000000000</RequestId>
  </ResponseMetadata>
</DescribeStacksResponse>"#,
                members
            );

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/xml")
                .body(Body::from(body))
                .unwrap()
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

    let input: DeleteStackInput = match parse_input(&body_str) {
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
        Ok(()) => {
            let body = r#"<DeleteStackResponse xmlns="http://cloudformation.amazonaws.com/doc/2010-05-15/">
  <ResponseMetadata>
    <RequestId>00000000-0000-0000-0000-000000000000</RequestId>
  </ResponseMetadata>
</DeleteStackResponse>"#;
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/xml")
                .body(Body::from(body))
                .unwrap()
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

#[derive(Debug, Deserialize)]
struct UpdateStackInput {
    #[serde(rename = "StackName")]
    stack_name: String,
    #[serde(rename = "TemplateBody")]
    template_body: Option<String>,
}

async fn handle_update_stack(state: Arc<CloudFormationState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    let input: UpdateStackInput = match parse_input(&body_str) {
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
        Ok(stack) => {
            let body = format!(
                r#"<UpdateStackResponse xmlns="http://cloudformation.amazonaws.com/doc/2010-05-15/">
  <UpdateStackResult>
    <StackId>{}</StackId>
  </UpdateStackResult>
  <ResponseMetadata>
    <RequestId>00000000-0000-0000-0000-000000000000</RequestId>
  </ResponseMetadata>
</UpdateStackResponse>"#,
                stack.stack_id
            );
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/xml")
                .body(Body::from(body))
                .unwrap()
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

async fn handle_list_stacks(state: Arc<CloudFormationState>, _body: Bytes) -> Response {
    let stacks = state.list_stacks();
    let mut members = String::new();
    for s in stacks {
        members.push_str(&format!(
            r#"        <member>
          <StackId>{}</StackId>
          <StackName>{}</StackName>
          <CreationTime>{}</CreationTime>
          <StackStatus>{}</StackStatus>
        </member>
"#,
            s.stack_id,
            s.stack_name,
            format!("{:.3}", s.creation_time as f64),
            s.status
        ));
    }

    let body = format!(
        r#"<ListStacksResponse xmlns="http://cloudformation.amazonaws.com/doc/2010-05-15/">
  <ListStacksResult>
    <StackSummaries>
{}    </StackSummaries>
  </ListStacksResult>
  <ResponseMetadata>
    <RequestId>00000000-0000-0000-0000-000000000000</RequestId>
  </ResponseMetadata>
</ListStacksResponse>"#,
        members
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/xml")
        .body(Body::from(body))
        .unwrap()
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ValidateTemplateInput {
    #[serde(rename = "TemplateBody")]
    template_body: Option<String>,
    #[serde(rename = "TemplateURL")]
    template_url: Option<String>,
}

async fn handle_validate_template(state: Arc<CloudFormationState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    let input: ValidateTemplateInput = match parse_input(&body_str) {
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
        Ok(result) => {
            let body = format!(
                r#"<ValidateTemplateResponse xmlns="http://cloudformation.amazonaws.com/doc/2010-05-15/">
  <ValidateTemplateResult>
    <Description>{}</Description>
  </ValidateTemplateResult>
  <ResponseMetadata>
    <RequestId>00000000-0000-0000-0000-000000000000</RequestId>
  </ResponseMetadata>
</ValidateTemplateResponse>"#,
                result.get("Description").and_then(|d| d.as_str()).unwrap_or("")
            );
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/xml")
                .body(Body::from(body))
                .unwrap()
        }
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

    let input: GetTemplateInput = match parse_input(&body_str) {
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
            let body = format!(
                r#"<GetTemplateResponse xmlns="http://cloudformation.amazonaws.com/doc/2010-05-15/">
  <GetTemplateResult>
    <TemplateBody>{}</TemplateBody>
  </GetTemplateResult>
  <ResponseMetadata>
    <RequestId>00000000-0000-0000-0000-000000000000</RequestId>
  </ResponseMetadata>
</GetTemplateResponse>"#,
                // XML escape the template body
                stack.template.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace("\"", "&quot;").replace("'", "&apos;")
            );
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/xml")
                .body(Body::from(body))
                .unwrap()
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

#[derive(Debug, Deserialize)]
struct DescribeStackResourcesInput {
    #[serde(rename = "StackName")]
    stack_name: String,
}

async fn handle_describe_stack_resources(state: Arc<CloudFormationState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    let input: DescribeStackResourcesInput = match parse_input(&body_str) {
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
            let mut members = String::new();
            for (i, name) in stack.resources.iter().enumerate() {
                members.push_str(&format!(
                    r#"        <member>
          <LogicalResourceId>{}</LogicalResourceId>
          <ResourceType>AWS::CloudFormation::Stack</ResourceType>
          <PhysicalResourceId>{}-{}</PhysicalResourceId>
          <ResourceStatus>CREATE_COMPLETE</ResourceStatus>
          <Timestamp>{}</Timestamp>
        </member>
"#,
                    name,
                    stack.stack_name,
                    i,
                    format!("{:.3}", stack.creation_time as f64)
                ));
            }

            let body = format!(
                r#"<DescribeStackResourcesResponse xmlns="http://cloudformation.amazonaws.com/doc/2010-05-15/">
  <DescribeStackResourcesResult>
    <StackResources>
{}    </StackResources>
  </DescribeStackResourcesResult>
  <ResponseMetadata>
    <RequestId>00000000-0000-0000-0000-000000000000</RequestId>
  </ResponseMetadata>
</DescribeStackResourcesResponse>"#,
                members
            );
            
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/xml")
                .body(Body::from(body))
                .unwrap()
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
    let body = format!(
        r#"<ErrorResponse xmlns="http://cloudformation.amazonaws.com/doc/2010-05-15/">
  <Error>
    <Type>Sender</Type>
    <Code>{}</Code>
    <Message>{}</Message>
  </Error>
  <RequestId>00000000-0000-0000-0000-000000000000</RequestId>
</ErrorResponse>"#,
        error_type, message
    );
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/xml")
        .body(Body::from(body))
        .unwrap()
}
