//! Service registry and initialization

use axum::{body::Body, http::Request, response::Response};

/// Registry of all AWS services
pub struct ServiceRegistry {
    s3_enabled: bool,
    dynamodb_enabled: bool,
    lambda_enabled: bool,
    // TODO: Add actual service implementations
    // s3: Option<RustStackS3>,
    // dynamodb: Option<DynamoDBProxy>,
    // lambda: Option<LambdaService>,
}

impl ServiceRegistry {
    pub fn new(s3: bool, dynamodb: bool, lambda: bool) -> Self {
        Self {
            s3_enabled: s3,
            dynamodb_enabled: dynamodb,
            lambda_enabled: lambda,
        }
    }

    pub async fn handle_s3(&self, _request: Request<Body>) -> Response<Body> {
        if !self.s3_enabled {
            return service_disabled_response("s3");
        }

        // TODO: Implement S3 handling via s3s
        Response::builder()
            .status(501)
            .body(Body::from(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Error>
    <Code>NotImplemented</Code>
    <Message>S3 service is not yet implemented</Message>
</Error>"#,
            ))
            .unwrap()
    }

    pub async fn handle_dynamodb(&self, _request: Request<Body>) -> Response<Body> {
        if !self.dynamodb_enabled {
            return service_disabled_response("dynamodb");
        }

        // TODO: Implement DynamoDB proxy
        Response::builder()
            .status(501)
            .header("content-type", "application/x-amz-json-1.0")
            .body(Body::from(
                r#"{"__type":"com.amazonaws.dynamodb.v20120810#ServiceUnavailableException","message":"DynamoDB service is not yet implemented"}"#,
            ))
            .unwrap()
    }

    pub async fn handle_lambda(&self, _request: Request<Body>) -> Response<Body> {
        if !self.lambda_enabled {
            return service_disabled_response("lambda");
        }

        // TODO: Implement Lambda handling
        Response::builder()
            .status(501)
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"Type":"Service","Message":"Lambda service is not yet implemented"}"#,
            ))
            .unwrap()
    }
}

fn service_disabled_response(service: &str) -> Response<Body> {
    Response::builder()
        .status(503)
        .body(Body::from(format!(
            "Service '{}' is disabled",
            service
        )))
        .unwrap()
}
