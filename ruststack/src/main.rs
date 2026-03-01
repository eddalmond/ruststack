//! RustStack - High-fidelity AWS Local Emulator
//!
//! RustStack provides local implementations of AWS services for development and testing.
//! Currently supports S3, DynamoDB, Lambda, and CloudWatch Logs.

mod cloudwatch;
mod config;
mod router;

use clap::Parser;
use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(name = "ruststack")]
#[command(about = "High-fidelity AWS local emulator", long_about = None)]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value = "4566", env = "RUSTSTACK_PORT")]
    port: u16,

    /// Host to bind to
    #[arg(long, default_value = "0.0.0.0", env = "RUSTSTACK_HOST")]
    host: String,

    /// Enable S3 service
    #[arg(long, default_value = "true", env = "RUSTSTACK_S3")]
    s3: bool,

    /// Enable DynamoDB service
    #[arg(long, default_value = "true", env = "RUSTSTACK_DYNAMODB")]
    dynamodb: bool,

    /// Enable Lambda service
    #[arg(long, default_value = "true", env = "RUSTSTACK_LAMBDA")]
    lambda: bool,

    /// Lambda executor mode: subprocess (default), docker, or auto
    #[arg(long, default_value = "subprocess", env = "RUSTSTACK_LAMBDA_EXECUTOR")]
    lambda_executor: String,

    /// Docker container TTL in seconds (for warm pool)
    #[arg(long, default_value = "300", env = "RUSTSTACK_LAMBDA_CONTAINER_TTL")]
    lambda_container_ttl: u64,

    /// Maximum concurrent Lambda containers
    #[arg(long, default_value = "10", env = "RUSTSTACK_LAMBDA_MAX_CONTAINERS")]
    lambda_max_containers: usize,

    /// Docker network mode (bridge or host)
    #[arg(long, default_value = "bridge", env = "RUSTSTACK_LAMBDA_NETWORK")]
    lambda_network: String,

    /// Data directory for persistence
    #[arg(long, env = "RUSTSTACK_DATA_DIR")]
    data_dir: Option<String>,

    /// Enable persistence (use RUSTSTACK_PERSISTENCE=1 or PERSISTENCE=1)
    #[arg(long, env = "RUSTSTACK_PERSISTENCE")]
    persistence: bool,

    /// Enable IAM enforcement for local requests
    #[arg(long, env = "RUSTSTACK_ENFORCE_IAM")]
    enforce_iam: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Load environment configuration (for SERVICES filtering and log level)
    let env_config = config::EnvConfig::from_env();

    // Determine effective service states (CLI args can override, or use SERVICES env)
    let s3_enabled = if !env_config.services.is_empty() {
        env_config.is_service_enabled("s3") && args.s3
    } else {
        args.s3
    };

    let dynamodb_enabled = if !env_config.services.is_empty() {
        env_config.is_service_enabled("dynamodb") && args.dynamodb
    } else {
        args.dynamodb
    };

    let lambda_enabled = if !env_config.services.is_empty() {
        env_config.is_service_enabled("lambda") && args.lambda
    } else {
        args.lambda
    };

    // Initialize tracing with configured log level
    let log_level_str = match env_config.log_level {
        tracing::Level::TRACE => "trace",
        tracing::Level::DEBUG => "debug",
        tracing::Level::INFO => "info",
        tracing::Level::WARN => "warn",
        tracing::Level::ERROR => "error",
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("ruststack={},tower_http=debug", log_level_str).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting RustStack...");
    info!("  Version: {}", env!("CARGO_PKG_VERSION"));
    info!("  Host: {}:{}", args.host, args.port);
    info!(
        "  Persistence: {}",
        if args.persistence || env_config.persistence {
            "enabled"
        } else {
            "disabled"
        }
    );
    info!(
        "  IAM Enforcement: {}",
        if args.enforce_iam || env_config.enforce_iam {
            "enabled"
        } else {
            "disabled"
        }
    );
    info!("  LocalStack Host: {}", env_config.localstack_host);
    info!("  Use SSL: {}", env_config.use_ssl);
    info!("  S3: {}", if s3_enabled { "enabled" } else { "disabled" });
    info!(
        "  DynamoDB: {}",
        if dynamodb_enabled {
            "enabled"
        } else {
            "disabled"
        }
    );
    info!(
        "  Lambda: {}",
        if lambda_enabled {
            "enabled"
        } else {
            "disabled"
        }
    );

    // Print SERVICES filter if set
    if !env_config.services.is_empty() {
        info!("  Services Filter: {}", env_config.services.join(", "));
    }

    // Parse Lambda executor mode
    let lambda_executor = args
        .lambda_executor
        .parse::<ruststack_lambda::ExecutorMode>()
        .unwrap_or_else(|_| {
            tracing::warn!(
                "Unknown lambda executor '{}', defaulting to subprocess",
                args.lambda_executor
            );
            ruststack_lambda::ExecutorMode::Subprocess
        });

    // Build Docker executor config
    let docker_config = ruststack_lambda::DockerExecutorConfig {
        container_ttl: std::time::Duration::from_secs(args.lambda_container_ttl),
        max_containers: args.lambda_max_containers,
        network_mode: args.lambda_network.clone(),
        ruststack_endpoint: if args.lambda_network == "host" {
            format!("http://localhost:{}", args.port)
        } else {
            format!("http://host.docker.internal:{}", args.port)
        },
    };

    // Determine if persistence is enabled (will be used in Phase 1)
    let persistence_enabled = args.persistence || env_config.persistence;

    // Get data directory
    let data_dir = args.data_dir.map(std::path::PathBuf::from);

    // Build services
    let state = router::AppState::new_with_config(
        s3_enabled,
        dynamodb_enabled,
        lambda_enabled,
        lambda_executor,
        docker_config,
        persistence_enabled,
        data_dir.as_deref(),
    );

    // Create router
    let app = router::create_router(state, env_config);

    // Start server
    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
