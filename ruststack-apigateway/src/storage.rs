//! API Gateway V2 in-memory storage

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// An HTTP API
#[derive(Debug, Clone)]
pub struct Api {
    pub api_id: String,
    pub name: String,
    pub protocol_type: String, // HTTP or WEBSOCKET
    pub api_endpoint: String,
    pub created_date: DateTime<Utc>,
    pub description: Option<String>,
    pub tags: HashMap<String, String>,
}

/// A route in an API
#[derive(Debug, Clone)]
pub struct Route {
    pub route_id: String,
    pub api_id: String,
    pub route_key: String, // e.g., "GET /items", "$default"
    pub target: Option<String>, // e.g., "integrations/abc123"
    pub authorization_type: Option<String>,
}

/// An integration (Lambda, HTTP proxy, etc.)
#[derive(Debug, Clone)]
pub struct Integration {
    pub integration_id: String,
    pub api_id: String,
    pub integration_type: String, // AWS_PROXY, HTTP_PROXY
    pub integration_uri: Option<String>, // Lambda ARN or HTTP URL
    pub integration_method: Option<String>,
    pub payload_format_version: String, // "1.0" or "2.0"
}

/// A stage (deployment environment)
#[derive(Debug, Clone)]
pub struct Stage {
    pub stage_name: String,
    pub api_id: String,
    pub auto_deploy: bool,
    pub created_date: DateTime<Utc>,
    pub description: Option<String>,
}

/// In-memory API Gateway storage
#[derive(Debug, Default)]
pub struct ApiGatewayStorage {
    apis: DashMap<String, Api>,
    routes: DashMap<String, Route>, // key: "{api_id}/{route_id}"
    integrations: DashMap<String, Integration>, // key: "{api_id}/{integration_id}"
    stages: DashMap<String, Stage>, // key: "{api_id}/{stage_name}"
}

impl ApiGatewayStorage {
    pub fn new() -> Self {
        Self::default()
    }

    // === APIs ===

    pub fn create_api(
        &self,
        name: &str,
        protocol_type: &str,
        description: Option<String>,
        tags: HashMap<String, String>,
    ) -> Api {
        let api_id = Self::generate_id();
        let api = Api {
            api_id: api_id.clone(),
            name: name.to_string(),
            protocol_type: protocol_type.to_string(),
            api_endpoint: format!("https://{}.execute-api.us-east-1.localhost.localstack.cloud:4566", api_id),
            created_date: Utc::now(),
            description,
            tags,
        };
        self.apis.insert(api_id, api.clone());
        api
    }

    pub fn get_api(&self, api_id: &str) -> Option<Api> {
        self.apis.get(api_id).map(|a| a.clone())
    }

    pub fn delete_api(&self, api_id: &str) -> Option<Api> {
        // Also delete associated routes, integrations, stages
        self.routes.retain(|k, _| !k.starts_with(&format!("{}/", api_id)));
        self.integrations.retain(|k, _| !k.starts_with(&format!("{}/", api_id)));
        self.stages.retain(|k, _| !k.starts_with(&format!("{}/", api_id)));
        self.apis.remove(api_id).map(|(_, v)| v)
    }

    pub fn list_apis(&self) -> Vec<Api> {
        self.apis.iter().map(|r| r.value().clone()).collect()
    }

    // === Routes ===

    pub fn create_route(&self, api_id: &str, route_key: &str, target: Option<String>) -> Option<Route> {
        if !self.apis.contains_key(api_id) {
            return None;
        }
        let route_id = Self::generate_id();
        let route = Route {
            route_id: route_id.clone(),
            api_id: api_id.to_string(),
            route_key: route_key.to_string(),
            target,
            authorization_type: None,
        };
        let key = format!("{}/{}", api_id, route_id);
        self.routes.insert(key, route.clone());
        Some(route)
    }

    pub fn get_route(&self, api_id: &str, route_id: &str) -> Option<Route> {
        let key = format!("{}/{}", api_id, route_id);
        self.routes.get(&key).map(|r| r.clone())
    }

    pub fn delete_route(&self, api_id: &str, route_id: &str) -> Option<Route> {
        let key = format!("{}/{}", api_id, route_id);
        self.routes.remove(&key).map(|(_, v)| v)
    }

    pub fn list_routes(&self, api_id: &str) -> Vec<Route> {
        self.routes
            .iter()
            .filter(|r| r.value().api_id == api_id)
            .map(|r| r.value().clone())
            .collect()
    }

    // === Integrations ===

    pub fn create_integration(
        &self,
        api_id: &str,
        integration_type: &str,
        integration_uri: Option<String>,
        integration_method: Option<String>,
        payload_format_version: Option<String>,
    ) -> Option<Integration> {
        if !self.apis.contains_key(api_id) {
            return None;
        }
        let integration_id = Self::generate_id();
        let integration = Integration {
            integration_id: integration_id.clone(),
            api_id: api_id.to_string(),
            integration_type: integration_type.to_string(),
            integration_uri,
            integration_method,
            payload_format_version: payload_format_version.unwrap_or_else(|| "2.0".to_string()),
        };
        let key = format!("{}/{}", api_id, integration_id);
        self.integrations.insert(key, integration.clone());
        Some(integration)
    }

    pub fn get_integration(&self, api_id: &str, integration_id: &str) -> Option<Integration> {
        let key = format!("{}/{}", api_id, integration_id);
        self.integrations.get(&key).map(|i| i.clone())
    }

    pub fn delete_integration(&self, api_id: &str, integration_id: &str) -> Option<Integration> {
        let key = format!("{}/{}", api_id, integration_id);
        self.integrations.remove(&key).map(|(_, v)| v)
    }

    pub fn list_integrations(&self, api_id: &str) -> Vec<Integration> {
        self.integrations
            .iter()
            .filter(|i| i.value().api_id == api_id)
            .map(|i| i.value().clone())
            .collect()
    }

    // === Stages ===

    pub fn create_stage(
        &self,
        api_id: &str,
        stage_name: &str,
        auto_deploy: bool,
        description: Option<String>,
    ) -> Option<Stage> {
        if !self.apis.contains_key(api_id) {
            return None;
        }
        let stage = Stage {
            stage_name: stage_name.to_string(),
            api_id: api_id.to_string(),
            auto_deploy,
            created_date: Utc::now(),
            description,
        };
        let key = format!("{}/{}", api_id, stage_name);
        self.stages.insert(key, stage.clone());
        Some(stage)
    }

    pub fn get_stage(&self, api_id: &str, stage_name: &str) -> Option<Stage> {
        let key = format!("{}/{}", api_id, stage_name);
        self.stages.get(&key).map(|s| s.clone())
    }

    pub fn delete_stage(&self, api_id: &str, stage_name: &str) -> Option<Stage> {
        let key = format!("{}/{}", api_id, stage_name);
        self.stages.remove(&key).map(|(_, v)| v)
    }

    pub fn list_stages(&self, api_id: &str) -> Vec<Stage> {
        self.stages
            .iter()
            .filter(|s| s.value().api_id == api_id)
            .map(|s| s.value().clone())
            .collect()
    }

    fn generate_id() -> String {
        // API Gateway uses alphanumeric IDs
        Uuid::new_v4().to_string().replace("-", "")[..10].to_string()
    }
}

/// State for API Gateway handlers
pub struct ApiGatewayState {
    pub storage: Arc<ApiGatewayStorage>,
}

impl ApiGatewayState {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(ApiGatewayStorage::new()),
        }
    }
}

impl Default for ApiGatewayState {
    fn default() -> Self {
        Self::new()
    }
}

/// API Gateway errors
#[derive(Debug, thiserror::Error)]
pub enum ApiGatewayError {
    #[error("API not found: {0}")]
    ApiNotFound(String),

    #[error("Route not found: {0}")]
    RouteNotFound(String),

    #[error("Integration not found: {0}")]
    IntegrationNotFound(String),

    #[error("Stage not found: {0}")]
    StageNotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_api() {
        let storage = ApiGatewayStorage::new();
        let api = storage.create_api("TestAPI", "HTTP", None, HashMap::new());
        
        assert_eq!(api.name, "TestAPI");
        assert_eq!(api.protocol_type, "HTTP");
        assert!(!api.api_id.is_empty());
    }

    #[test]
    fn test_create_route_and_integration() {
        let storage = ApiGatewayStorage::new();
        let api = storage.create_api("TestAPI", "HTTP", None, HashMap::new());
        
        let integration = storage
            .create_integration(&api.api_id, "AWS_PROXY", Some("arn:aws:lambda:...".to_string()), None, None)
            .unwrap();
        
        let route = storage
            .create_route(&api.api_id, "GET /test", Some(format!("integrations/{}", integration.integration_id)))
            .unwrap();
        
        assert_eq!(route.route_key, "GET /test");
        assert!(route.target.unwrap().contains(&integration.integration_id));
    }

    #[test]
    fn test_delete_api_cascades() {
        let storage = ApiGatewayStorage::new();
        let api = storage.create_api("TestAPI", "HTTP", None, HashMap::new());
        storage.create_route(&api.api_id, "GET /", None);
        storage.create_integration(&api.api_id, "AWS_PROXY", None, None, None);
        storage.create_stage(&api.api_id, "$default", true, None);
        
        storage.delete_api(&api.api_id);
        
        assert!(storage.list_routes(&api.api_id).is_empty());
        assert!(storage.list_integrations(&api.api_id).is_empty());
        assert!(storage.list_stages(&api.api_id).is_empty());
    }
}
