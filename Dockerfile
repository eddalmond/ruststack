# Multi-stage Dockerfile for RustStack
# Builds from source - no pre-built binary required

# ============================================
# Stage 1: Build the Rust binary
# ============================================
FROM rust:1.75-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests first for better caching
COPY Cargo.toml Cargo.lock ./
COPY ruststack/Cargo.toml ruststack/
COPY ruststack-core/Cargo.toml ruststack-core/
COPY ruststack-auth/Cargo.toml ruststack-auth/
COPY ruststack-s3/Cargo.toml ruststack-s3/
COPY ruststack-dynamodb/Cargo.toml ruststack-dynamodb/
COPY ruststack-lambda/Cargo.toml ruststack-lambda/

# Create dummy source files to build dependencies
RUN mkdir -p ruststack/src ruststack-core/src ruststack-auth/src \
    ruststack-s3/src ruststack-dynamodb/src ruststack-lambda/src && \
    echo "fn main() {}" > ruststack/src/main.rs && \
    echo "pub fn dummy() {}" > ruststack-core/src/lib.rs && \
    echo "pub fn dummy() {}" > ruststack-auth/src/lib.rs && \
    echo "pub fn dummy() {}" > ruststack-s3/src/lib.rs && \
    echo "pub fn dummy() {}" > ruststack-dynamodb/src/lib.rs && \
    echo "pub fn dummy() {}" > ruststack-lambda/src/lib.rs

# Build dependencies only (cached layer)
RUN cargo build --release && rm -rf target/release/.fingerprint/ruststack*

# Copy actual source code
COPY ruststack/src ruststack/src
COPY ruststack-core/src ruststack-core/src
COPY ruststack-auth/src ruststack-auth/src
COPY ruststack-s3/src ruststack-s3/src
COPY ruststack-dynamodb/src ruststack-dynamodb/src
COPY ruststack-lambda/src ruststack-lambda/src

# Build the actual binary
RUN cargo build --release

# ============================================
# Stage 2: Runtime image
# ============================================
FROM debian:bookworm-slim

# Install runtime dependencies
# - ca-certificates: for HTTPS if needed
# - python3: for Lambda function execution
# - curl: for health checks
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    python3 \
    python3-pip \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/target/release/ruststack /usr/local/bin/ruststack

# Create non-root user
RUN useradd -m -s /bin/bash ruststack
USER ruststack

EXPOSE 4566

ENV RUST_LOG=info

# Health check
HEALTHCHECK --interval=5s --timeout=3s --start-period=2s --retries=3 \
    CMD curl -f http://localhost:4566/health || exit 1

ENTRYPOINT ["ruststack"]
CMD ["--host", "0.0.0.0", "--port", "4566"]
