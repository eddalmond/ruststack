//! HTTP handlers for SNS

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::Response,
};
use bytes::Bytes;
use std::sync::Arc;
use tracing::{info, warn};

use crate::storage::{SnsError, SnsState};

/// Handle SNS requests based on X-Amz-Target header
pub async fn handle_request(
    State(state): State<Arc<SnsState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let target = headers
        .get("x-amz-target")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    info!(target = %target, "SNS request");

    match target {
        "AmazonSNS.CreateTopic" => handle_create_topic(state, body).await,
        "AmazonSNS.DeleteTopic" => handle_delete_topic(state, body).await,
        "AmazonSNS.ListTopics" => handle_list_topics(state, body).await,
        "AmazonSNS.GetTopicAttributes" => handle_get_topic_attributes(state, body).await,
        "AmazonSNS.Subscribe" => handle_subscribe(state, body).await,
        "AmazonSNS.Unsubscribe" => handle_unsubscribe(state, body).await,
        "AmazonSNS.ListSubscriptions" => handle_list_subscriptions(state, body).await,
        "AmazonSNS.ListSubscriptionsByTopic" => {
            handle_list_subscriptions_by_topic(state, body).await
        }
        "AmazonSNS.Publish" => handle_publish(state, body).await,
        _ => {
            warn!(target = %target, "Unknown SNS operation");
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

async fn handle_create_topic(state: Arc<SnsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);
    let topic_name = match get_xml_value(&body_str, "Name") {
        Some(name) => name,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "MissingParameter",
                "Name is required",
            );
        }
    };

    match state.create_topic(&topic_name) {
        Ok(topic) => xml_response(
            StatusCode::OK,
            &format!(
                r#"<CreateTopicResponse>
                <CreateTopicResult>
                    <TopicArn>{}</TopicArn>
                </CreateTopicResult>
            </CreateTopicResponse>"#,
                topic.arn
            ),
        ),
        Err(SnsError::TopicAlreadyExists(name)) => match state.get_topic(&name) {
            Ok(topic) => xml_response(
                StatusCode::OK,
                &format!(
                    r#"<CreateTopicResponse>
                        <CreateTopicResult>
                            <TopicArn>{}</TopicArn>
                        </CreateTopicResult>
                    </CreateTopicResponse>"#,
                    topic.arn
                ),
            ),
            Err(_) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "UnknownError",
                "Failed to get topic",
            ),
        },
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "UnknownError",
            &e.to_string(),
        ),
    }
}

async fn handle_delete_topic(state: Arc<SnsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);
    let topic_arn = match get_xml_value(&body_str, "TopicArn") {
        Some(arn) => arn,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "MissingParameter",
                "TopicArn is required",
            );
        }
    };

    let topic_name = topic_arn.split(':').next_back().unwrap_or(&topic_arn);

    match state.delete_topic(topic_name) {
        Ok(()) => xml_response(
            StatusCode::OK,
            r#"<DeleteTopicResponse></DeleteTopicResponse>"#,
        ),
        Err(SnsError::TopicNotFound(_)) => error_response(
            StatusCode::NOT_FOUND,
            "TopicNotFound",
            "Topic does not exist",
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "UnknownError",
            &e.to_string(),
        ),
    }
}

async fn handle_list_topics(state: Arc<SnsState>, _body: Bytes) -> Response {
    let topics = state.list_topics();

    let mut xml = String::from("<ListTopicsResponse><ListTopicsResult>");
    for topic in topics {
        xml.push_str("<Topics>");
        xml.push_str(&format!("<TopicArn>{}</TopicArn>", topic.arn));
        xml.push_str("</Topics>");
    }
    xml.push_str("</ListTopicsResult></ListTopicsResponse>");

    xml_response(StatusCode::OK, &xml)
}

async fn handle_get_topic_attributes(state: Arc<SnsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);
    let topic_arn = match get_xml_value(&body_str, "TopicArn") {
        Some(arn) => arn,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "MissingParameter",
                "TopicArn is required",
            );
        }
    };

    let topic_name = topic_arn.split(':').next_back().unwrap_or(&topic_arn);

    match state.get_topic(topic_name) {
        Ok(topic) => xml_response(
            StatusCode::OK,
            &format!(
                r#"<GetTopicAttributesResponse>
                <GetTopicAttributesResult>
                    <TopicArn>{}</TopicArn>
                    <Owner>000000000000</Owner>
                    <TopicCreatedTimestamp>{}</TopicCreatedTimestamp>
                </GetTopicAttributesResult>
            </GetTopicAttributesResponse>"#,
                topic.arn, topic.created_timestamp
            ),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "UnknownError",
            &e.to_string(),
        ),
    }
}

async fn handle_subscribe(state: Arc<SnsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    let topic_arn = match get_xml_value(&body_str, "TopicArn") {
        Some(arn) => arn,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "MissingParameter",
                "TopicArn is required",
            );
        }
    };

    let protocol = match get_xml_value(&body_str, "Protocol") {
        Some(p) => p,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "MissingParameter",
                "Protocol is required",
            );
        }
    };

    let endpoint = match get_xml_value(&body_str, "Endpoint") {
        Some(e) => e,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "MissingParameter",
                "Endpoint is required",
            );
        }
    };

    let topic_name = topic_arn.split(':').next_back().unwrap_or(&topic_arn);

    match state.subscribe(topic_name, &protocol, &endpoint) {
        Ok(sub) => xml_response(
            StatusCode::OK,
            &format!(
                r#"<SubscribeResponse>
                <SubscribeResult>
                    <SubscriptionArn>{}</SubscriptionArn>
                </SubscribeResult>
            </SubscribeResponse>"#,
                sub.arn()
            ),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "UnknownError",
            &e.to_string(),
        ),
    }
}

async fn handle_unsubscribe(state: Arc<SnsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);
    let subscription_arn = match get_xml_value(&body_str, "SubscriptionArn") {
        Some(arn) => arn,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "MissingParameter",
                "SubscriptionArn is required",
            );
        }
    };

    match state.unsubscribe(&subscription_arn) {
        Ok(()) => xml_response(
            StatusCode::OK,
            r#"<UnsubscribeResponse></UnsubscribeResponse>"#,
        ),
        Err(SnsError::SubscriptionNotFound(_)) => error_response(
            StatusCode::NOT_FOUND,
            "SubscriptionNotFound",
            "Subscription does not exist",
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "UnknownError",
            &e.to_string(),
        ),
    }
}

async fn handle_list_subscriptions(state: Arc<SnsState>, _body: Bytes) -> Response {
    let subscriptions = state.list_all_subscriptions();

    let mut xml = String::from("<ListSubscriptionsResponse><ListSubscriptionsResult>");
    for (topic_name, sub) in subscriptions {
        xml.push_str("<Subscriptions>");
        xml.push_str(&format!(
            "<TopicArn>arn:aws:sns:us-east-1:000000000000:{}</TopicArn>",
            topic_name
        ));
        xml.push_str(&format!(
            "<Protocol>{}</Protocol>",
            match sub {
                crate::storage::Subscription::Sqs { .. } => "sqs",
                crate::storage::Subscription::Http { .. } => "http",
                crate::storage::Subscription::Https { .. } => "https",
                crate::storage::Subscription::Email { .. } => "email",
                crate::storage::Subscription::Lambda { .. } => "lambda",
            }
        ));
        xml.push_str("<SubscriptionArn>");
        xml.push_str(sub.arn());
        xml.push_str("</SubscriptionArn>");
        xml.push_str("<Owner>000000000000</Owner>");
        xml.push_str("</Subscriptions>");
    }
    xml.push_str("</ListSubscriptionsResult></ListSubscriptionsResponse>");

    xml_response(StatusCode::OK, &xml)
}

async fn handle_list_subscriptions_by_topic(state: Arc<SnsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);
    let topic_arn = match get_xml_value(&body_str, "TopicArn") {
        Some(arn) => arn,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "MissingParameter",
                "TopicArn is required",
            );
        }
    };

    let topic_name = topic_arn.split(':').next_back().unwrap_or(&topic_arn);

    match state.list_subscriptions(topic_name) {
        Ok(subs) => {
            let mut xml =
                String::from("<ListSubscriptionsByTopicResponse><ListSubscriptionsByTopicResult>");
            for sub in subs {
                xml.push_str("<Subscriptions>");
                xml.push_str(&format!("<TopicArn>{}</TopicArn>", topic_arn));
                xml.push_str(&format!(
                    "<Protocol>{}</Protocol>",
                    match sub {
                        crate::storage::Subscription::Sqs { .. } => "sqs",
                        crate::storage::Subscription::Http { .. } => "http",
                        crate::storage::Subscription::Https { .. } => "https",
                        crate::storage::Subscription::Email { .. } => "email",
                        crate::storage::Subscription::Lambda { .. } => "lambda",
                    }
                ));
                xml.push_str("<SubscriptionArn>");
                xml.push_str(sub.arn());
                xml.push_str("</SubscriptionArn>");
                xml.push_str("<Owner>000000000000</Owner>");
                xml.push_str("</Subscriptions>");
            }
            xml.push_str("</ListSubscriptionsByTopicResult></ListSubscriptionsByTopicResponse>");
            xml_response(StatusCode::OK, &xml)
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "UnknownError",
            &e.to_string(),
        ),
    }
}

async fn handle_publish(state: Arc<SnsState>, body: Bytes) -> Response {
    let body_str = String::from_utf8_lossy(&body);

    let topic_arn = match get_xml_value(&body_str, "TopicArn") {
        Some(arn) => arn,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "MissingParameter",
                "TopicArn is required",
            );
        }
    };

    let message = match get_xml_value(&body_str, "Message") {
        Some(m) => m,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "MissingParameter",
                "Message is required",
            );
        }
    };

    let subject = get_xml_value(&body_str, "Subject");

    let topic_name = topic_arn.split(':').next_back().unwrap_or(&topic_arn);

    match state.publish(topic_name, &message, subject.as_deref()) {
        Ok(message_id) => xml_response(
            StatusCode::OK,
            &format!(
                r#"<PublishResponse>
                <PublishResult>
                    <MessageId>{}</MessageId>
                </PublishResult>
            </PublishResponse>"#,
                message_id
            ),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "UnknownError",
            &e.to_string(),
        ),
    }
}

// === XML Helpers ===

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
        r#"<ErrorResponse xmlns="http://sns.amazonaws.com/doc/2010-03-31/">
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
