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

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info", env = "RUSTSTACK_LOG_LEVEL")]
    log_level: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("ruststack={},tower_http=debug", args.log_level).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

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

    info!("Starting RustStack...");
    info!("  S3: {}", if args.s3 { "enabled" } else { "disabled" });
    info!(
        "  DynamoDB: {}",
        if args.dynamodb { "enabled" } else { "disabled" }
    );
    info!(
        "  Lambda: {} (executor: {:?})",
        if args.lambda { "enabled" } else { "disabled" },
        lambda_executor
    );

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

    // Build services
    let state = router::AppState::new_with_lambda_config(
        args.s3,
        args.dynamodb,
        args.lambda,
        lambda_executor,
        docker_config,
    );

    // Create router
    let app = router::create_router(state);

    // Start server
    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
