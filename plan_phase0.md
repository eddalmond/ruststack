# Phase 0: Foundation & Multiplexing Engine

**Objective:** Establish the core HTTP gateway, environment variable configuration, and routing middleware for RustStack.

**Timeline:** Weeks 1-2

---

## Task 0.1: Project Initialization & Config Management

### Overview
Ensure the configuration manager properly parses and enforces all required environment variables. The current implementation has partial support but needs enhancement.

### Environment Variables to Support

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SERVICES` | Comma-delimited | All enabled | Comma-delimited list. If present, only boot listed modules |
| `DEBUG` / `LS_LOG` | String | "info" | Map to tracing log levels (trace, debug, info, warn, error) |
| `PERSISTENCE` / `RUSTSTACK_PERSISTENCE` | Boolean | false | If 1, initialize SQLite/File I/O; otherwise, use volatile memory |
| `LOCALSTACK_HOST` | String | localhost.localstack.cloud:4566 | Host used in mock URLs |
| `USE_SSL` | Boolean | false | Toggle for returning https in mock URLs |

### Steps for LLM Agent

1. **Read existing config implementation:**
   - Read `ruststack/src/config.rs`
   - Read `ruststack/src/main.rs` to see current CLI args

2. **Add missing environment variables:**
   - Edit `ruststack/src/config.rs` to add:
     - `SERVICES` parsing that filters which services start
     - `LOCALSTACK_HOST` for mock URL generation  
     - `USE_SSL` toggle
     - Update log level handling to support `DEBUG` and `LS_LOG`

3. **Create config module for environment variable parsing:**
   ```rust
   // Add to config.rs
   #[derive(Debug, Clone)]
   pub struct EnvConfig {
       pub services: Vec<String>,
       pub log_level: tracing::Level,
       pub persistence: bool,
       pub localstack_host: String,
       pub use_ssl: bool,
   }
   
   impl EnvConfig {
       pub fn from_env() -> Self { ... }
   }
   ```

4. **Update main.rs to use new config:**
   - Parse env vars at startup
   - Pass configuration to service initialization
   - Print enabled services at startup

5. **Acceptance Test:**
   - Run `RUSTSTACK_S3=false RUSTSTACK_LOG_LEVEL=debug cargo run -- -p 4566`
   - Verify S3 is disabled and debug logging works

---

## Task 0.2: The Gateway Router (axum)

### Overview
The router already exists and implements intelligent request multiplexing. This task focuses on improving it and adding protocol decoding middleware.

### Current State
- Router binds to `0.0.0.0:4566` ✓
- Multiplexing via `X-Amz-Target` header ✓
- Host header routing for S3 virtual-hosted styles - needs enhancement

### Steps for LLM Agent

1. **Read current router implementation:**
   - Read `ruststack/src/router.rs` fully

2. **Enhance protocol detection middleware:**
   
   a. Create `ruststack/src/middleware.rs`:
   ```rust
   use axum::{
       extract::Request,
       middleware::Next,
       response::Response,
   };
   
   /// AWS Protocol types
   #[derive(Debug, Clone, Copy)]
   pub enum AwsProtocol {
       RestJson,    // aws.rest-json-1.1
       Query,       // aws.query
       Json,        // aws.json-1.0
       Unknown,
   }
   
   impl AwsProtocol {
       pub fn from_content_type(ct: &str) -> Self {
           // Parse Content-Type header for protocol
       }
   }
   ```

   b. Add middleware to router in `router.rs`:
   ```rust
   Router::new()
       .layer(middleware::from_fn(extract_aws_protocol))
       // ... existing routes
   ```

3. **Enhance S3 virtual-hosted style routing:**
   
   The router should detect `bucket.host:4566` style requests:
   
   ```rust
   async fn extract_bucket_from_host(
       headers: &HeaderMap,
       uri: &Uri,
   ) -> Option<String> {
       // Check Host header for bucket.subdomain pattern
   }
   ```

4. **Test routing:**
   - Start server with `cargo run`
   - Test DynamoDB: `curl -H "X-Amz-Target: DynamoDB_20120810.ListTables" ...`
   - Test S3 path-style: `curl http://localhost:4566/bucket/key`
   - Test S3 virtual-hosted: `curl http://bucket.localhost:4566/key`

5. **Acceptance Test:**
   - `aws s3 ls --endpoint-url http://localhost:4566` returns proper response
   - S3 operations work with both path and virtual-hosted styles

---

## Task 0.3: Mock SigV4 Interceptor

### Overview
Implement a middleware that validates the structural presence of AWS Signature Version 4 headers. This prevents official SDKs from throwing pre-flight errors.

### Steps for LLM Agent

1. **Read existing auth implementation:**
   - Read `ruststack-auth/src/lib.rs`
   - Read `ruststack-auth/src/sigv4.rs`

2. **Understand SigV4 structure:**
   - Required headers: `Authorization`, `X-Amz-Date`, `X-Amz-Credential`, `X-Amz-SignedHeaders`
   - The middleware should validate presence (not cryptographic correctness)

3. **Create SigV4 validation middleware:**

   Create `ruststack-auth/src/middleware.rs`:
   ```rust
   use axum::{
       extract::Request,
       http::{header, HeaderName, HeaderValue},
       middleware::Next,
       response::Response,
       body::Body,
   };
   
   /// Validates SigV4 headers are present (structural validation only)
   pub async fn validate_sigv4(request: Request<Body>, next: Next) -> Response {
       let headers = request.headers();
       
       // Check for SigV4 indicators
       let has_authorization = headers.contains_key("authorization");
       let has_date = headers.contains_key("x-amz-date");
       
       if has_authorization || has_date {
           // Continue to actual handler
           return next.run(request).await;
       }
       
       // For non-signed requests, continue without modification
       next.run(request).await
   }
   ```

4. **Add to router:**
   In `ruststack/src/router.rs`:
   ```rust
   use ruststack_auth::middleware::validate_sigv4;
   
   Router::new()
       .layer(middleware::from_fn(validate_sigv4))
       // ... rest of router
   ```

5. **Handle SigV4 errors properly:**
   
   When SigV4 is present but invalid, return proper AWS error:
   ```rust
   if has_authorization && !is_valid_sigv4(headers) {
       return Response::builder()
           .status(StatusCode::FORBIDDEN)
           .header(header::CONTENT_TYPE, "application/json")
           .body(Body::from(r#"{"__type":"InvalidSignatureException"}"#))
           .unwrap();
   }
   ```

6. **Test with AWS CLI:**
   ```bash
   aws configure set aws_access_key_id test
   aws configure set aws_secret_access_key test
   aws s3 ls --endpoint-url http://localhost:4566
   # Should NOT return SignatureDoesNotMatch error
   ```

7. **Acceptance Test:**
   - AWS SDK requests with SigV4 headers return 200 (or proper service error)
   - Requests without SigV4 continue normally
   - Invalid SigV4 returns `InvalidSignatureException` not generic 500

---

## Phase 0 Acceptance Criteria Summary

| Criterion | Test Command | Expected Result |
|-----------|--------------|-----------------|
| Router binds to port 4566 | `cargo run` | Server starts on 0.0.0.0:4566 |
| Environment config works | `RUSTSTACK_S3=false cargo run` | S3 service disabled in output |
| DynamoDB routing | `curl -H "X-Amz-Target: DynamoDB_20120810.ListTables" -X POST localhost:4566` | Returns DynamoDB response |
| S3 path-style routing | `curl localhost:4566/my-bucket/my-key` | Routes to S3 handler |
| SigV4 validation | AWS CLI commands | No SigV4 errors |
| Logging works | `RUSTSTACK_LOG_LEVEL=debug cargo run` | Debug logs visible |

---

## Notes for LLM Agent

- **File locations:** All new code should go in appropriate crates:
  - Core config: `ruststack/src/config.rs`
  - Router: `ruststack/src/router.rs`
  - Auth middleware: `ruststack-auth/src/middleware.rs`
  
- **Testing approach:**
  - Use `cargo test` to run existing tests
  - Manual testing with `curl` and AWS CLI
  - Add unit tests for new middleware

- **Dependencies already available:**
  - axum, tower, tower-http
  - tokio, tracing
  - All AWS-related crates already in workspace
