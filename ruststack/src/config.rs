//! Configuration management

use serde::Deserialize;
use std::path::PathBuf;

/// Main configuration structure
#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,

    #[serde(default)]
    pub services: ServicesConfig,

    #[serde(default)]
    pub storage: StorageConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_host")]
    pub host: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            host: default_host(),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct ServicesConfig {
    #[serde(default = "default_true")]
    pub s3: bool,

    #[serde(default = "default_true")]
    pub dynamodb: bool,

    #[serde(default = "default_true")]
    pub lambda: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum StorageConfig {
    #[serde(rename = "ephemeral")]
    Ephemeral,

    #[serde(rename = "filesystem")]
    FileSystem { path: PathBuf },
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self::Ephemeral
    }
}

fn default_port() -> u16 {
    4566
}

fn default_host() -> String {
    "0.0.0.0".to_string()
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
