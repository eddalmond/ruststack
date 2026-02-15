//! RustStack - High-fidelity AWS Local Emulator
//!
//! RustStack provides local implementations of AWS services for development and testing.
//! Currently supports S3, DynamoDB, and Lambda.

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
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("ruststack={},tower_http=debug", args.log_level).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting RustStack...");
    info!("  S3: {}", if args.s3 { "enabled" } else { "disabled" });
    info!(
        "  DynamoDB: {}",
        if args.dynamodb { "enabled" } else { "disabled" }
    );
    info!(
        "  Lambda: {}",
        if args.lambda { "enabled" } else { "disabled" }
    );

    // Build services
    let state = router::AppState::new(args.s3, args.dynamodb, args.lambda);

    // Create router
    let app = router::create_router(state);

    // Start server
    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
