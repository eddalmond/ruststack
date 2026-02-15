//! Lambda function models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported Lambda runtimes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Runtime {
    #[serde(rename = "python3.9")]
    Python39,
    #[serde(rename = "python3.10")]
    Python310,
    #[serde(rename = "python3.11")]
    Python311,
    #[serde(rename = "python3.12")]
    Python312,
    #[serde(rename = "nodejs18.x")]
    Nodejs18,
    #[serde(rename = "nodejs20.x")]
    Nodejs20,
    #[serde(rename = "provided.al2")]
    ProvidedAl2,
    #[serde(rename = "provided.al2023")]
    ProvidedAl2023,
}

impl Runtime {
    /// Get the Docker image for this runtime
    pub fn docker_image(&self) -> &'static str {
        match self {
            Self::Python39 => "public.ecr.aws/lambda/python:3.9",
            Self::Python310 => "public.ecr.aws/lambda/python:3.10",
            Self::Python311 => "public.ecr.aws/lambda/python:3.11",
            Self::Python312 => "public.ecr.aws/lambda/python:3.12",
            Self::Nodejs18 => "public.ecr.aws/lambda/nodejs:18",
            Self::Nodejs20 => "public.ecr.aws/lambda/nodejs:20",
            Self::ProvidedAl2 => "public.ecr.aws/lambda/provided:al2",
            Self::ProvidedAl2023 => "public.ecr.aws/lambda/provided:al2023",
        }
    }

    /// Parse runtime string
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "python3.9" => Some(Self::Python39),
            "python3.10" => Some(Self::Python310),
            "python3.11" => Some(Self::Python311),
            "python3.12" => Some(Self::Python312),
            "nodejs18.x" => Some(Self::Nodejs18),
            "nodejs20.x" => Some(Self::Nodejs20),
            "provided.al2" => Some(Self::ProvidedAl2),
            "provided.al2023" => Some(Self::ProvidedAl2023),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Python39 => "python3.9",
            Self::Python310 => "python3.10",
            Self::Python311 => "python3.11",
            Self::Python312 => "python3.12",
            Self::Nodejs18 => "nodejs18.x",
            Self::Nodejs20 => "nodejs20.x",
            Self::ProvidedAl2 => "provided.al2",
            Self::ProvidedAl2023 => "provided.al2023",
        }
    }
}

/// Function configuration
#[derive(Debug, Clone)]
pub struct FunctionConfig {
    pub function_name: String,
    pub runtime: Runtime,
    pub handler: String,
    pub role: String,
    pub memory_size: i32,
    pub timeout: i32,
    pub environment: HashMap<String, String>,
    pub description: Option<String>,
}

impl Default for FunctionConfig {
    fn default() -> Self {
        Self {
            function_name: String::new(),
            runtime: Runtime::Python312,
            handler: "lambda_function.lambda_handler".to_string(),
            role: "arn:aws:iam::000000000000:role/lambda-role".to_string(),
            memory_size: 128,
            timeout: 3,
            environment: HashMap::new(),
            description: None,
        }
    }
}

/// Function code source
#[derive(Debug, Clone)]
pub enum FunctionCode {
    /// Inline zip (base64 encoded)
    ZipFile(Vec<u8>),
    /// S3 reference
    S3 {
        bucket: String,
        key: String,
        version: Option<String>,
    },
}

/// Lambda function state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FunctionState {
    Pending,
    Active,
    Inactive,
    Failed,
}

/// A Lambda function
#[derive(Debug, Clone)]
pub struct Function {
    pub config: FunctionConfig,
    pub code: FunctionCode,
    pub code_sha256: String,
    pub code_size: i64,
    pub state: FunctionState,
    pub last_modified: DateTime<Utc>,
    pub version: String,
    pub arn: String,
}

impl Function {
    /// Create a new function
    pub fn new(
        config: FunctionConfig,
        code: FunctionCode,
        code_sha256: String,
        code_size: i64,
    ) -> Self {
        let arn = format!(
            "arn:aws:lambda:us-east-1:000000000000:function:{}",
            config.function_name
        );

        Self {
            config,
            code,
            code_sha256,
            code_size,
            state: FunctionState::Active,
            last_modified: Utc::now(),
            version: "$LATEST".to_string(),
            arn,
        }
    }

    /// Get function ARN
    pub fn arn(&self) -> &str {
        &self.arn
    }

    /// Get qualified ARN (with version/alias)
    pub fn qualified_arn(&self) -> String {
        format!("{}:{}", self.arn, self.version)
    }
}

/// API Gateway event structure (v1 format)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiGatewayEvent {
    pub resource: String,
    pub path: String,
    pub http_method: String,
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub multi_value_headers: HashMap<String, Vec<String>>,
    pub query_string_parameters: Option<HashMap<String, String>>,
    #[serde(default)]
    pub multi_value_query_string_parameters: Option<HashMap<String, Vec<String>>>,
    pub path_parameters: Option<HashMap<String, String>>,
    pub stage_variables: Option<HashMap<String, String>>,
    pub request_context: ApiGatewayRequestContext,
    pub body: Option<String>,
    pub is_base64_encoded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiGatewayRequestContext {
    pub account_id: String,
    pub api_id: String,
    pub http_method: String,
    pub identity: ApiGatewayIdentity,
    pub path: String,
    pub stage: String,
    pub request_id: String,
    pub request_time: String,
    pub request_time_epoch: i64,
    pub resource_id: String,
    pub resource_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ApiGatewayIdentity {
    pub source_ip: String,
    pub user_agent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cognito_identity_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cognito_identity_pool_id: Option<String>,
}

/// API Gateway response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiGatewayResponse {
    pub status_code: i32,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub multi_value_headers: HashMap<String, Vec<String>>,
    pub body: Option<String>,
    #[serde(default)]
    pub is_base64_encoded: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_docker_image() {
        assert_eq!(
            Runtime::Python312.docker_image(),
            "public.ecr.aws/lambda/python:3.12"
        );
    }

    #[test]
    fn test_api_gateway_event_serialization() {
        let event = ApiGatewayEvent {
            resource: "/test".to_string(),
            path: "/test".to_string(),
            http_method: "GET".to_string(),
            headers: HashMap::new(),
            multi_value_headers: HashMap::new(),
            query_string_parameters: None,
            multi_value_query_string_parameters: None,
            path_parameters: None,
            stage_variables: None,
            request_context: ApiGatewayRequestContext {
                account_id: "123456789012".to_string(),
                api_id: "abc123".to_string(),
                http_method: "GET".to_string(),
                identity: ApiGatewayIdentity::default(),
                path: "/test".to_string(),
                stage: "prod".to_string(),
                request_id: "req-123".to_string(),
                request_time: "01/Jan/2024:00:00:00 +0000".to_string(),
                request_time_epoch: 1704067200,
                resource_id: "res-123".to_string(),
                resource_path: "/test".to_string(),
            },
            body: None,
            is_base64_encoded: false,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("httpMethod"));
        assert!(json.contains("requestContext"));
    }
}
