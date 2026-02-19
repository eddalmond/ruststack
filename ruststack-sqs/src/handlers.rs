//! HTTP handlers for SQS

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::Response,
};
use bytes::Bytes;
use std::sync::Arc;
use tracing::{info, warn};

use crate::storage::{SqsError, SqsState};

/// Handle SQS requests based on X-Amz-Target header
pub async fn handle_request(
    State(state): State<Arc<SqsState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let target = headers
        .get("x-amz-target")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    info!(target = %target, "SQS request");

    match target {
        "AmazonSQS.CreateQueue" => handle_create_queue(state, body).await,
        "AmazonSQS.DeleteQueue" => handle_delete_queue(state, body).await,
        "AmazonSQS.ListQueues" => handle_list_queues(state, body).await,
        "AmazonSQS.GetQueueUrl" => handle_get_queue_url(state, body).await,
        "AmazonSQS.SendMessage" => handle_send_message(state, body).await,
        "AmazonSQS.ReceiveMessage" => handle_receive_message(state, body).await,
        "AmazonSQS.DeleteMessage" => handle_delete_message(state, body).await,
        "AmazonSQS.GetQueueAttributes" => handle_get_queue_attributes(state, body).await,
        "AmazonSQS.SetQueueAttributes" => handle_set_queue_attributes(state, body).await,
        _ => {
            warn!(target = %target, "Unknown SQS operation");
            error_response(
                StatusCode::BAD_REQUEST,
                "UnknownOperationException",
                &format!("Unknown operation: {}", target),
            )
        }
    }
}

// === Simple XML parsing helpers ===

fn get_xml_value(body: &str, tag: &str) -> Option<String> {
    let open_tag = format!("<{}>", tag);
    let close_tag = format!("</{}>", tag);

    if let Some(start) = body.find(&open_tag) {
        let value_start = start + open_tag.len();
        if let Some(end) = body[value_start..].find(&close_tag) {
            let value = &body[value_start..value_start + end];
            return Some(value.trim().to_string());
        }
    }
    None
}

// === Handlers ===

async fn handle_create_queue(state: Arc<SqsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);
    let queue_name = match get_xml_value(&body_str, "QueueName") {
        Some(name) => name,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "MissingParameter",
                "QueueName is required",
            );
        }
    };

    match state.create_queue(&queue_name) {
        Ok(queue) => xml_response(
            StatusCode::OK,
            &format!(
                r#"<CreateQueueResponse>
                <CreateQueueResult>
                    <QueueUrl>{}</QueueUrl>
                </CreateQueueResult>
            </CreateQueueResponse>"#,
                queue.url
            ),
        ),
        Err(SqsError::QueueAlreadyExists(name)) => match state.get_queue(&name) {
            Ok(queue) => xml_response(
                StatusCode::OK,
                &format!(
                    r#"<CreateQueueResponse>
                        <CreateQueueResult>
                            <QueueUrl>{}</QueueUrl>
                        </CreateQueueResult>
                    </CreateQueueResponse>"#,
                    queue.url
                ),
            ),
            Err(_) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "UnknownError",
                "Failed to get queue",
            ),
        },
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "UnknownError",
            &e.to_string(),
        ),
    }
}

async fn handle_delete_queue(state: Arc<SqsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);
    let queue_url = match get_xml_value(&body_str, "QueueUrl") {
        Some(url) => url,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "MissingParameter",
                "QueueUrl is required",
            );
        }
    };

    let queue_name = queue_url.split('/').next_back().unwrap_or(&queue_url);

    match state.delete_queue(queue_name) {
        Ok(()) => xml_response(
            StatusCode::OK,
            r#"<DeleteQueueResponse></DeleteQueueResponse>"#,
        ),
        Err(SqsError::QueueNotFound(_)) => error_response(
            StatusCode::BAD_REQUEST,
            "QueueDoesNotExist",
            "Queue does not exist",
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "UnknownError",
            &e.to_string(),
        ),
    }
}

async fn handle_list_queues(state: Arc<SqsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);
    let prefix = get_xml_value(&body_str, "QueueNamePrefix");

    let queues = state.list_queues(prefix.as_deref());

    let mut xml = String::from("<ListQueuesResponse><ListQueuesResult>");
    for url in queues {
        xml.push_str(&format!("<QueueUrl>{}</QueueUrl>", url));
    }
    xml.push_str("</ListQueuesResult></ListQueuesResponse>");

    xml_response(StatusCode::OK, &xml)
}

async fn handle_get_queue_url(state: Arc<SqsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);
    let queue_name = match get_xml_value(&body_str, "QueueName") {
        Some(name) => name,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "MissingParameter",
                "QueueName is required",
            );
        }
    };

    match state.get_queue(&queue_name) {
        Ok(queue) => xml_response(
            StatusCode::OK,
            &format!(
                r#"<GetQueueUrlResponse>
                <GetQueueUrlResult>
                    <QueueUrl>{}</QueueUrl>
                </GetQueueUrlResult>
            </GetQueueUrlResponse>"#,
                queue.url
            ),
        ),
        Err(SqsError::QueueNotFound(_)) => error_response(
            StatusCode::BAD_REQUEST,
            "QueueDoesNotExist",
            "Queue does not exist",
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "UnknownError",
            &e.to_string(),
        ),
    }
}

async fn handle_send_message(state: Arc<SqsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);
    let queue_url = match get_xml_value(&body_str, "QueueUrl") {
        Some(url) => url,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "MissingParameter",
                "QueueUrl is required",
            );
        }
    };
    let message_body = match get_xml_value(&body_str, "MessageBody") {
        Some(body) => body,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "MissingParameter",
                "MessageBody is required",
            );
        }
    };

    let queue_name = queue_url.split('/').next_back().unwrap_or(&queue_url);

    match state.send_message(queue_name, message_body) {
        Ok(msg) => xml_response(
            StatusCode::OK,
            &format!(
                r#"<SendMessageResponse>
                <SendMessageResult>
                    <MD5OfMessageBody>{}</MD5OfMessageBody>
                    <MessageId>{}</MessageId>
                    <QueueUrl>{}</QueueUrl>
                </SendMessageResult>
            </SendMessageResponse>"#,
                msg.md5_of_body, msg.message_id, queue_url
            ),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "UnknownError",
            &e.to_string(),
        ),
    }
}

async fn handle_receive_message(state: Arc<SqsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);
    let queue_url = match get_xml_value(&body_str, "QueueUrl") {
        Some(url) => url,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "MissingParameter",
                "QueueUrl is required",
            );
        }
    };
    let max_messages = get_xml_value(&body_str, "MaxNumberOfMessages")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);

    let queue_name = queue_url.split('/').next_back().unwrap_or(&queue_url);

    match state.receive_message(queue_name, max_messages) {
        Ok(messages) => {
            let mut xml = String::from("<ReceiveMessageResponse><ReceiveMessageResult>");
            for msg in messages {
                xml.push_str("<Message>");
                xml.push_str(&format!("<MessageId>{}</MessageId>", msg.message_id));
                xml.push_str(&format!(
                    "<ReceiptHandle>{}</ReceiptHandle>",
                    msg.receipt_handle
                ));
                xml.push_str(&format!("<MD5OfBody>{}</MD5OfBody>", msg.md5_of_body));
                xml.push_str(&format!("<Body>{}</Body>", escape_xml(&msg.body)));
                xml.push_str("<Attributes>");
                xml.push_str(&format!(
                    "<ApproximateReceiveCount>{}</ApproximateReceiveCount>",
                    msg.approximate_receive_count
                ));
                xml.push_str("</Attributes>");
                xml.push_str("</Message>");
            }
            xml.push_str("</ReceiveMessageResult></ReceiveMessageResponse>");
            xml_response(StatusCode::OK, &xml)
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "UnknownError",
            &e.to_string(),
        ),
    }
}

async fn handle_delete_message(state: Arc<SqsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);
    let queue_url = match get_xml_value(&body_str, "QueueUrl") {
        Some(url) => url,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "MissingParameter",
                "QueueUrl is required",
            );
        }
    };
    let receipt_handle = match get_xml_value(&body_str, "ReceiptHandle") {
        Some(handle) => handle,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "MissingParameter",
                "ReceiptHandle is required",
            );
        }
    };

    let queue_name = queue_url.split('/').next_back().unwrap_or(&queue_url);

    match state.delete_message(queue_name, &receipt_handle) {
        Ok(()) => xml_response(
            StatusCode::OK,
            r#"<DeleteMessageResponse></DeleteMessageResponse>"#,
        ),
        Err(SqsError::MessageNotFound(_)) => error_response(
            StatusCode::BAD_REQUEST,
            "ReceiptHandleIsInvalid",
            "Receipt handle is invalid",
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "UnknownError",
            &e.to_string(),
        ),
    }
}

async fn handle_get_queue_attributes(state: Arc<SqsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);
    let queue_url = match get_xml_value(&body_str, "QueueUrl") {
        Some(url) => url,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "MissingParameter",
                "QueueUrl is required",
            );
        }
    };

    let queue_name = queue_url.split('/').next_back().unwrap_or(&queue_url);

    match state.get_queue(queue_name) {
        Ok(queue) => xml_response(
            StatusCode::OK,
            &format!(
                r#"<GetQueueAttributesResponse>
                <GetQueueAttributesResult>
                    <QueueArn>{}</QueueArn>
                    <VisibilityTimeout>{}</VisibilityTimeout>
                    <ReceiveMessageWaitTimeSeconds>{}</ReceiveMessageWaitTimeSeconds>
                </GetQueueAttributesResult>
            </GetQueueAttributesResponse>"#,
                queue.arn, queue.visibility_timeout, queue.receive_message_wait_time_seconds
            ),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "UnknownError",
            &e.to_string(),
        ),
    }
}

async fn handle_set_queue_attributes(_state: Arc<SqsState>, _body: Bytes) -> Response {
    // For now, just acknowledge
    xml_response(
        StatusCode::OK,
        r#"<SetQueueAttributesResponse></SetQueueAttributesResponse>"#,
    )
}

// === XML Helpers ===

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn xml_response(status: StatusCode, body: &str) -> Response {
    let mut response = Response::new(Body::from(body.to_string()));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/xml"),
    );
    response
}

fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    let xml = format!(
        r#"<ErrorResponse xmlns="http://queue.amazonaws.com/doc/2012-11-05/">
  <Error>
    <Type>Sender</Type>
    <Code>{}</Code>
    <Message>{}</Message>
    <Detail/>
  </Error>
  <RequestId>{}</RequestId>
</ErrorResponse>"#,
        code,
        message,
        uuid::Uuid::new_v4()
    );
    let mut response = Response::new(Body::from(xml));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/xml"),
    );
    response
}
