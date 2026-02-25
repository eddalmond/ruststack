//! Configuration management
//!
//! Supports configuration via:
//! - Environment variables (RUSTSTACK_*)
//! - Config file (ruststack.toml)
//! - Direct Rust API

use serde::Deserialize;
use std::path::PathBuf;
use tracing::Level;

/// Main configuration structure
#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,

    #[serde(default)]
    pub services: ServicesConfig,

    #[serde(default)]
    pub storage: StorageConfig,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ServerConfig {
    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_localstack_host")]
    pub localstack_host: String,

    #[serde(default = "default_use_ssl")]
    pub use_ssl: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            host: default_host(),
            localstack_host: default_localstack_host(),
            use_ssl: default_use_ssl(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct EnvConfig {
    pub services: Vec<String>,
    pub log_level: Level,
    pub persistence: bool,
    pub localstack_host: String,
    pub use_ssl: bool,
}

impl Default for EnvConfig {
    fn default() -> Self {
        Self {
            services: vec![],
            log_level: Level::INFO,
            persistence: false,
            localstack_host: default_localstack_host(),
            use_ssl: false,
        }
    }
}

impl EnvConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        // Parse SERVICES env var (comma-delimited list)
        let services = std::env::var("RUSTSTACK_SERVICES")
            .or_else(|_| std::env::var("SERVICES"))
            .map(|s| {
                s.split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        // Parse log level from DEBUG or LS_LOG or RUSTSTACK_LOG_LEVEL
        let log_level = std::env::var("RUSTSTACK_LOG_LEVEL")
            .or_else(|_| std::env::var("LS_LOG"))
            .or_else(|_| std::env::var("DEBUG"))
            .map(|s| match s.to_lowercase().as_str() {
                "trace" => Level::TRACE,
                "debug" => Level::DEBUG,
                "info" => Level::INFO,
                "warn" | "warning" => Level::WARN,
                "error" => Level::ERROR,
                _ => Level::INFO,
            })
            .unwrap_or(Level::INFO);

        // Parse persistence
        let persistence = std::env::var("RUSTSTACK_PERSISTENCE")
            .or_else(|_| std::env::var("PERSISTENCE"))
            .map(|s| s == "1" || s.to_lowercase() == "true")
            .unwrap_or(false);

        // LOCALSTACK_HOST
        let localstack_host =
            std::env::var("LOCALSTACK_HOST").unwrap_or_else(|_| default_localstack_host());

        // USE_SSL
        let use_ssl = std::env::var("RUSTSTACK_USE_SSL")
            .or_else(|_| std::env::var("USE_SSL"))
            .map(|s| s == "1" || s.to_lowercase() == "true")
            .unwrap_or(false);

        Self {
            services,
            log_level,
            persistence,
            localstack_host,
            use_ssl,
        }
    }

    /// Check if a specific service should be enabled
    pub fn is_service_enabled(&self, service: &str) -> bool {
        if self.services.is_empty() {
            true // All services enabled by default
        } else {
            self.services.iter().any(|s| s == service)
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
pub struct ServicesConfig {
    #[serde(default = "default_true")]
    pub s3: bool,

    #[serde(default = "default_true")]
    pub dynamodb: bool,

    #[serde(default = "default_true")]
    pub lambda: bool,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
#[serde(tag = "type")]
pub enum StorageConfig {
    #[serde(rename = "ephemeral")]
    #[default]
    Ephemeral,

    #[serde(rename = "filesystem")]
    FileSystem { path: PathBuf },
}

// Default derived via #[default] attribute above

fn default_port() -> u16 {
    4566
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_localstack_host() -> String {
    "localhost.localstack.cloud:4566".to_string()
}

fn default_use_ssl() -> bool {
    false
}

fn default_true() -> bool {
    true
}

impl Config {
    /// Load configuration from file and environment
    #[allow(dead_code)]
    pub fn load() -> anyhow::Result<Self> {
        let config = config::Config::builder()
            .add_source(config::File::with_name("ruststack").required(false))
            .add_source(config::Environment::with_prefix("RUSTSTACK"))
            .build()?;

        Ok(config.try_deserialize::<Config>()?)
    }
}
