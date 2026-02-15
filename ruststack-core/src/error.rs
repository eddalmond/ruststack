//! AWS error types and formatting

use serde::Serialize;
use thiserror::Error;

/// Common AWS error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    // Common
    AccessDenied,
    InvalidAccessKeyId,
    InvalidSignature,
    RequestTimeTooSkewed,
    ServiceUnavailable,

    // S3 specific
    NoSuchBucket,
    NoSuchKey,
    BucketAlreadyExists,
    BucketAlreadyOwnedByYou,
    BucketNotEmpty,
    InvalidBucketName,
    InvalidArgument,
    EntityTooLarge,
    EntityTooSmall,
    InvalidPart,
    InvalidPartOrder,
    NoSuchUpload,

    // DynamoDB specific
    ResourceNotFoundException,
    ResourceInUseException,
    ValidationException,
    ConditionalCheckFailedException,
    ProvisionedThroughputExceededException,
    TransactionConflictException,

    // Lambda specific
    ResourceNotFound,
    InvalidParameterValue,
    ServiceException,
    TooManyRequestsException,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AccessDenied => "AccessDenied",
            Self::InvalidAccessKeyId => "InvalidAccessKeyId",
            Self::InvalidSignature => "SignatureDoesNotMatch",
            Self::RequestTimeTooSkewed => "RequestTimeTooSkewed",
            Self::ServiceUnavailable => "ServiceUnavailable",
            Self::NoSuchBucket => "NoSuchBucket",
            Self::NoSuchKey => "NoSuchKey",
            Self::BucketAlreadyExists => "BucketAlreadyExists",
            Self::BucketAlreadyOwnedByYou => "BucketAlreadyOwnedByYou",
            Self::BucketNotEmpty => "BucketNotEmpty",
            Self::InvalidBucketName => "InvalidBucketName",
            Self::InvalidArgument => "InvalidArgument",
            Self::EntityTooLarge => "EntityTooLarge",
            Self::EntityTooSmall => "EntityTooSmall",
            Self::InvalidPart => "InvalidPart",
            Self::InvalidPartOrder => "InvalidPartOrder",
            Self::NoSuchUpload => "NoSuchUpload",
            Self::ResourceNotFoundException => "ResourceNotFoundException",
            Self::ResourceInUseException => "ResourceInUseException",
            Self::ValidationException => "ValidationException",
            Self::ConditionalCheckFailedException => "ConditionalCheckFailedException",
            Self::ProvisionedThroughputExceededException => "ProvisionedThroughputExceededException",
            Self::TransactionConflictException => "TransactionConflictException",
            Self::ResourceNotFound => "ResourceNotFound",
            Self::InvalidParameterValue => "InvalidParameterValueException",
            Self::ServiceException => "ServiceException",
            Self::TooManyRequestsException => "TooManyRequestsException",
        }
    }

    pub fn http_status(&self) -> u16 {
        match self {
            Self::AccessDenied | Self::InvalidAccessKeyId | Self::InvalidSignature => 403,
            Self::NoSuchBucket | Self::NoSuchKey | Self::NoSuchUpload
            | Self::ResourceNotFoundException | Self::ResourceNotFound => 404,
            Self::BucketAlreadyExists | Self::BucketAlreadyOwnedByYou
            | Self::ResourceInUseException | Self::TransactionConflictException => 409,
            Self::InvalidArgument | Self::InvalidBucketName | Self::ValidationException
            | Self::InvalidParameterValue | Self::InvalidPart | Self::InvalidPartOrder => 400,
            Self::EntityTooLarge => 400,
            Self::EntityTooSmall => 400,
            Self::BucketNotEmpty => 409,
            Self::RequestTimeTooSkewed => 403,
            Self::ConditionalCheckFailedException => 400,
            Self::ProvisionedThroughputExceededException | Self::TooManyRequestsException => 429,
            Self::ServiceUnavailable | Self::ServiceException => 500,
        }
    }
}

/// AWS-style error
#[derive(Debug, Error)]
#[error("{code}: {message}")]
pub struct AwsError {
    pub code: ErrorCode,
    pub message: String,
    pub resource: Option<String>,
    pub request_id: String,
}

impl AwsError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            resource: None,
            request_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    pub fn with_resource(mut self, resource: impl Into<String>) -> Self {
        self.resource = Some(resource.into());
        self
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = request_id.into();
        self
    }

    /// Format as S3-style XML error
    pub fn to_xml(&self) -> String {
        let resource_line = self
            .resource
            .as_ref()
            .map(|r| format!("    <Resource>{}</Resource>\n", r))
            .unwrap_or_default();

        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Error>
    <Code>{}</Code>
    <Message>{}</Message>
{}    <RequestId>{}</RequestId>
</Error>"#,
            self.code.as_str(),
            self.message,
            resource_line,
            self.request_id
        )
    }

    /// Format as DynamoDB-style JSON error
    pub fn to_json(&self) -> String {
        #[derive(Serialize)]
        struct JsonError<'a> {
            #[serde(rename = "__type")]
            error_type: String,
            message: &'a str,
        }

        let error = JsonError {
            error_type: format!("com.amazonaws.dynamodb.v20120810#{}", self.code.as_str()),
            message: &self.message,
        };

        serde_json::to_string(&error).unwrap_or_else(|_| {
            format!(r#"{{"__type":"{}","message":"{}"}}"#, self.code.as_str(), self.message)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_xml_format() {
        let error = AwsError::new(ErrorCode::NoSuchBucket, "The specified bucket does not exist")
            .with_resource("my-bucket")
            .with_request_id("test-request-id");

        let xml = error.to_xml();
        assert!(xml.contains("<Code>NoSuchBucket</Code>"));
        assert!(xml.contains("<Resource>my-bucket</Resource>"));
        assert!(xml.contains("<RequestId>test-request-id</RequestId>"));
    }

    #[test]
    fn test_error_json_format() {
        let error = AwsError::new(ErrorCode::ResourceNotFoundException, "Table not found");

        let json = error.to_json();
        assert!(json.contains("ResourceNotFoundException"));
        assert!(json.contains("Table not found"));
    }
}
