# RustStack Architecture Design (Focused Scope)

## Overview

RustStack is a Rust-based AWS local emulator optimized for **integration testing Flask/Lambda applications**. The architecture prioritizes:

1. **Error Fidelity** - Exact AWS error codes and formats
2. **Flask Compatibility** - API Gateway v1 event format
3. **Developer Speed** - Fast startup, minimal setup
4. **Simplicity** - Focused scope, clear code

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      RustStack Server                           │
│                      (Single Binary)                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │              HTTP Gateway (Axum)                        │   │
│   │  • Port 4566 (all services)                            │   │
│   │  • Service detection from headers/path                  │   │
│   │  • Request ID generation                                │   │
│   └─────────────────────────────────────────────────────────┘   │
│                              │                                   │
│          ┌───────────────────┼───────────────────┐              │
│          ▼                   ▼                   ▼              │
│   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐      │
│   │     S3      │     │  DynamoDB   │     │   Lambda    │      │
│   │  (s3s)      │     │  (Proxy)    │     │ (Docker)    │      │
│   └──────┬──────┘     └──────┬──────┘     └──────┬──────┘      │
│          │                   │                   │              │
│   ┌──────▼──────┐     ┌──────▼──────┐     ┌──────▼──────┐      │
│   │  In-Memory  │     │  DynamoDB   │     │  Container  │      │
│   │   Storage   │     │    Local    │     │    Pool     │      │
│   │  (DashMap)  │     │   (Java)    │     │  (Bollard)  │      │
│   └─────────────┘     └─────────────┘     └─────────────┘      │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Component Design

### 1. HTTP Gateway

Single entry point on port 4566. Service detection:

```rust
fn detect_service(headers: &HeaderMap, path: &str) -> Service {
    // DynamoDB: X-Amz-Target header starts with "DynamoDB"
    if let Some(target) = headers.get("x-amz-target") {
        if target.to_str().unwrap_or("").starts_with("DynamoDB") {
            return Service::DynamoDB;
        }
    }

    // Lambda: path starts with /2015-03-31/functions
    if path.starts_with("/2015-03-31/functions") {
        return Service::Lambda;
    }

    // Default: S3
    Service::S3
}
```

### 2. S3 Service

Built on **s3s** framework with custom in-memory storage:

```rust
pub struct RustStackS3 {
    storage: EphemeralStorage,
}

#[async_trait]
impl s3s::S3 for RustStackS3 {
    async fn get_object(&self, req: S3Request<GetObjectInput>)
        -> S3Result<S3Response<GetObjectOutput>>
    {
        let bucket = &req.input.bucket;
        let key = &req.input.key;

        let object = self.storage
            .get_object(bucket, key, None)
            .await
            .map_err(|e| match e {
                StorageError::BucketNotFound(_) => s3_error!(NoSuchBucket),
                StorageError::ObjectNotFound { .. } => s3_error!(NoSuchKey),
                _ => s3_error!(InternalError),
            })?;

        // Handle range request
        let (body, content_range) = if let Some(range) = &req.input.range {
            self.apply_range(object.data, range)?
        } else {
            (object.data, None)
        };

        Ok(S3Response::new(GetObjectOutput {
            body: Some(StreamingBlob::from(body)),
            content_length: Some(body.len() as i64),
            content_range,
            e_tag: Some(object.etag),
            last_modified: Some(object.last_modified.into()),
            ..Default::default()
        }))
    }

    // ... other operations
}
```

**Storage Backend:**

```rust
pub struct EphemeralStorage {
    buckets: DashMap<String, Bucket>,
}

struct Bucket {
    objects: DashMap<String, StoredObject>,
    created_at: DateTime<Utc>,
}

struct StoredObject {
    data: Bytes,
    etag: String,  // MD5 hex with quotes
    last_modified: DateTime<Utc>,
    content_type: Option<String>,
    metadata: HashMap<String, String>,
}
```

### 3. DynamoDB Service

**Proxy architecture** to DynamoDB Local:

```rust
pub struct DynamoDBService {
    server: DynamoDBLocalServer,
    client: reqwest::Client,
}

impl DynamoDBService {
    pub async fn handle(&self, action: &str, body: Bytes) -> Result<Response, DynamoDBError> {
        // Forward to DynamoDB Local
        let response = self.client
            .post(&format!("http://localhost:{}/", self.server.port()))
            .header("X-Amz-Target", format!("DynamoDB_20120810.{}", action))
            .header("Content-Type", "application/x-amz-json-1.0")
            .body(body)
            .send()
            .await?;

        // Fix ARNs in response
        let body = self.fix_arns(response.bytes().await?)?;

        Ok(Response::builder()
            .status(response.status())
            .header("Content-Type", "application/x-amz-json-1.0")
            .body(body.into())?)
    }

    fn fix_arns(&self, body: Bytes) -> Result<Bytes, DynamoDBError> {
        // Replace "arn:aws:dynamodb:ddblocal:000000000000:"
        // With    "arn:aws:dynamodb:us-east-1:000000000000:"
        let s = String::from_utf8_lossy(&body);
        let fixed = s.replace("ddblocal", "us-east-1");
        Ok(Bytes::from(fixed.into_owned()))
    }
}
```

### 4. Lambda Service

**Container-based execution:**

```rust
pub struct LambdaService {
    docker: Docker,
    functions: DashMap<String, Function>,
    containers: DashMap<String, Container>,  // Warm container pool
}

impl LambdaService {
    pub async fn invoke(
        &self,
        function_name: &str,
        event: Bytes,
    ) -> Result<InvocationResult, LambdaError> {
        let function = self.functions.get(function_name)
            .ok_or(LambdaError::FunctionNotFound)?;

        // Get or create container
        let container = self.get_or_create_container(&function).await?;

        // Send invocation via Runtime API
        let result = container.invoke(event).await?;

        Ok(result)
    }

    async fn get_or_create_container(&self, function: &Function) -> Result<Container, LambdaError> {
        // Check for warm container
        if let Some(container) = self.containers.get(&function.name) {
            return Ok(container.clone());
        }

        // Create new container
        let container = self.docker.create_container(
            CreateContainerOptions { name: &format!("ruststack-{}", function.name) },
            Config {
                image: Some(function.runtime.docker_image()),
                env: Some(self.build_env(&function)),
                // Mount function code
                host_config: Some(HostConfig {
                    binds: Some(vec![
                        format!("{}:/var/task:ro", function.code_path),
                    ]),
                    ..Default::default()
                }),
                cmd: Some(vec![function.handler.clone()]),
                ..Default::default()
            }
        ).await?;

        // Start container
        self.docker.start_container(&container.id, None).await?;

        // Wait for Runtime API to be ready
        self.wait_for_runtime_api(&container).await?;

        Ok(container)
    }
}
```

---

## Data Flow

### S3 GetObject

```
Client                RustStack              Storage
  │                      │                      │
  │  GET /bucket/key     │                      │
  │─────────────────────>│                      │
  │                      │  get_object()        │
  │                      │─────────────────────>│
  │                      │<─────────────────────│
  │                      │  StoredObject        │
  │  200 OK + body       │                      │
  │<─────────────────────│                      │
```

### DynamoDB GetItem

```
Client                RustStack           DynamoDB Local
  │                      │                      │
  │  POST (GetItem)      │                      │
  │─────────────────────>│                      │
  │                      │  Forward             │
  │                      │─────────────────────>│
  │                      │<─────────────────────│
  │                      │  Fix ARNs            │
  │  200 OK + Item       │                      │
  │<─────────────────────│                      │
```

### Lambda Invoke

```
Client               RustStack              Container
  │                      │                      │
  │  POST /invoke        │                      │
  │─────────────────────>│                      │
  │                      │  Get container       │
  │                      │─────────────────────>│
  │                      │  Runtime API: next   │
  │                      │<─────────────────────│
  │                      │  Deliver event       │
  │                      │─────────────────────>│
  │                      │  Runtime API: resp   │
  │                      │<─────────────────────│
  │  200 OK + result     │                      │
  │<─────────────────────│                      │
```

---

## Error Handling

### S3 Errors

```rust
pub enum S3ErrorCode {
    NoSuchBucket,
    NoSuchKey,
    BucketAlreadyExists,
    BucketNotEmpty,
    InvalidArgument,
    AccessDenied,
}

impl S3ErrorCode {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::NoSuchBucket | Self::NoSuchKey => StatusCode::NOT_FOUND,
            Self::BucketAlreadyExists | Self::BucketNotEmpty => StatusCode::CONFLICT,
            Self::InvalidArgument => StatusCode::BAD_REQUEST,
            Self::AccessDenied => StatusCode::FORBIDDEN,
        }
    }

    pub fn to_xml(&self, message: &str, resource: Option<&str>) -> String {
        format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<Error>
    <Code>{}</Code>
    <Message>{}</Message>
    {}
    <RequestId>{}</RequestId>
</Error>"#,
            self.as_str(),
            message,
            resource.map(|r| format!("<Resource>{}</Resource>", r)).unwrap_or_default(),
            uuid::Uuid::new_v4(),
        )
    }
}
```

### DynamoDB Errors

DynamoDB Local handles errors natively. We pass through with status code.

### Lambda Errors

```rust
pub struct LambdaError {
    pub error_message: String,
    pub error_type: String,
}

impl LambdaError {
    pub fn to_json(&self) -> String {
        serde_json::json!({
            "errorMessage": self.error_message,
            "errorType": self.error_type
        }).to_string()
    }
}
```

---

## Configuration

Minimal configuration via environment variables:

```rust
pub struct Config {
    /// Port to listen on (default: 4566)
    pub port: u16,

    /// Enable S3 service (default: true)
    pub s3_enabled: bool,

    /// Enable DynamoDB service (default: true)
    pub dynamodb_enabled: bool,

    /// Enable Lambda service (default: true)
    pub lambda_enabled: bool,

    /// DynamoDB Local JAR path (default: ./DynamoDBLocal.jar)
    pub dynamodb_local_path: PathBuf,

    /// Docker socket (default: /var/run/docker.sock)
    pub docker_socket: PathBuf,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            port: env::var("RUSTSTACK_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(4566),
            s3_enabled: env::var("RUSTSTACK_S3")
                .map(|s| s != "false" && s != "0")
                .unwrap_or(true),
            // ...
        }
    }
}
```

---

## Dependency Choices

### Web Framework: Axum

- Modern, async-first
- Tower middleware ecosystem
- Excellent performance
- Used by s3s

### S3: s3s

- Generated from Smithy models
- SigV4 auth built-in
- Clean trait-based design
- Active development

### DynamoDB: DynamoDB Local (Java)

- Official AWS tool
- 100% expression compatibility
- Battle-tested
- Only option for full fidelity

### Lambda: Bollard

- Pure Rust Docker client
- Async
- Well-maintained

### Storage: DashMap

- Lock-free concurrent HashMap
- Better than RwLock<HashMap>
- Simple API

---

## Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| S3 GetObject latency | < 5ms | In-memory |
| DynamoDB GetItem latency | < 10ms | DDB Local overhead |
| Lambda cold start | < 3s | Container startup |
| Lambda warm invoke | < 100ms | Reused container |
| Memory (idle) | < 50MB | Before DDB Local |
| Memory (with DDB Local) | < 300MB | Java overhead |

---

## Security Model

RustStack is for **local development only**. Security is minimal:

- No authentication by default (accepts any credentials)
- All requests succeed if structurally valid
- No TLS (use reverse proxy if needed)
- No IAM policy evaluation

This is intentional - security testing should use AWS directly.

---

## Testing Architecture

### Unit Tests

Each module has inline tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_put_get_object() {
        let storage = EphemeralStorage::new();
        storage.create_bucket("test").await.unwrap();

        storage.put_object("test", "key", Bytes::from("data"), Default::default())
            .await.unwrap();

        let obj = storage.get_object("test", "key", None).await.unwrap();
        assert_eq!(&obj.data[..], b"data");
    }
}
```

### Integration Tests

Test against running RustStack with AWS SDK:

```rust
// tests/integration/s3.rs
#[tokio::test]
async fn test_s3_error_codes() {
    let client = create_test_client().await;

    let err = client.get_object()
        .bucket("nonexistent-bucket")
        .key("key")
        .send()
        .await
        .unwrap_err();

    // Verify error code
    assert!(err.to_string().contains("NoSuchBucket"));
}
```

### Compatibility Matrix

| Operation | RustStack | LocalStack | AWS |
|-----------|-----------|------------|-----|
| S3 GetObject | ✓ | ✓ | ✓ |
| S3 NoSuchKey error | ✓ | ✓ | ✓ |
| DynamoDB GetItem | ✓ | ✓ | ✓ |
| DynamoDB ConditionFailed | ✓ | ✓ | ✓ |
| Lambda Invoke | ✓ | ✓ | ✓ |

---

## Deployment

### Binary

```bash
# Build
cargo build --release

# Run
./target/release/ruststack
```

### Docker

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y openjdk-17-jre-headless
COPY --from=builder /app/target/release/ruststack /usr/local/bin/
COPY DynamoDBLocal.jar /opt/dynamodb-local/
ENV RUSTSTACK_DYNAMODB_LOCAL_PATH=/opt/dynamodb-local/DynamoDBLocal.jar
EXPOSE 4566
CMD ["ruststack"]
```

### docker-compose

```yaml
version: '3.8'
services:
  ruststack:
    image: ruststack:latest
    ports:
      - "4566:4566"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
    environment:
      - RUSTSTACK_PORT=4566
```
