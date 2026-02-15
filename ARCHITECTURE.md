# RustStack Architecture Design

## Overview

RustStack is a high-fidelity AWS local emulator written in Rust, focusing on S3, DynamoDB, and Lambda services. The architecture prioritizes:

1. **AWS API Fidelity** - Exact error codes, response formats, edge case handling
2. **Performance** - Rust's zero-cost abstractions, async I/O
3. **Modularity** - Pluggable service implementations and storage backends
4. **Developer Experience** - Easy setup, fast startup, minimal dependencies

---

## High-Level Architecture

```
┌────────────────────────────────────────────────────────────────────┐
│                         RustStack Server                           │
├────────────────────────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                      HTTP Gateway (Axum)                     │  │
│  │  • Virtual-hosted & Path-style routing                       │  │
│  │  • Request authentication (SigV4, SigV2)                     │  │
│  │  • Request/Response logging                                  │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                               │                                     │
│           ┌───────────────────┼───────────────────┐                │
│           ▼                   ▼                   ▼                │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐      │
│  │   S3 Service    │ │ DynamoDB Service│ │ Lambda Service  │      │
│  │   (s3s-based)   │ │   (Proxy mode)  │ │   (Container)   │      │
│  └────────┬────────┘ └────────┬────────┘ └────────┬────────┘      │
│           │                   │                   │                │
│  ┌────────▼────────┐ ┌────────▼────────┐ ┌────────▼────────┐      │
│  │ Storage Backend │ │  DDB Local JVM  │ │  Docker Runtime │      │
│  │ (Pluggable)     │ │  or Native      │ │  + Runtime API  │      │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘      │
├────────────────────────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    State Management Layer                    │  │
│  │  • Multi-account/region support                              │  │
│  │  • Persistence (optional)                                    │  │
│  │  • Event bus for cross-service communication                 │  │
│  └──────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────┘
```

---

## Project Structure

```
ruststack/
├── Cargo.toml                 # Workspace definition
├── ruststack/                 # Main binary crate
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs            # Entry point, CLI
│       ├── config.rs          # Configuration loading
│       ├── server.rs          # HTTP server setup
│       └── router.rs          # Service routing
│
├── ruststack-core/            # Core types and traits
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── account.rs         # Account/region context
│       ├── error.rs           # AWS error types
│       ├── request_id.rs      # Request ID generation
│       └── event.rs           # Cross-service events
│
├── ruststack-s3/              # S3 implementation
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── service.rs         # S3 trait implementation
│       ├── storage/
│       │   ├── mod.rs
│       │   ├── ephemeral.rs   # In-memory storage
│       │   ├── filesystem.rs  # File-based storage
│       │   └── traits.rs      # Storage abstraction
│       ├── versioning.rs      # Version management
│       ├── multipart.rs       # Multipart upload handling
│       ├── notifications.rs   # Event notifications
│       ├── presigned.rs       # Presigned URL handling
│       └── validation.rs      # Request validation
│
├── ruststack-dynamodb/        # DynamoDB implementation
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── proxy.rs           # DynamoDB Local proxy
│       ├── server.rs          # DDB Local process management
│       ├── streams.rs         # DynamoDB Streams support
│       ├── global_tables.rs   # Global table handling
│       └── models.rs          # Additional state
│
├── ruststack-lambda/          # Lambda implementation
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── service.rs         # Lambda API implementation
│       ├── function.rs        # Function models
│       ├── invocation/
│       │   ├── mod.rs
│       │   ├── manager.rs     # Invocation orchestration
│       │   ├── container.rs   # Docker container management
│       │   └── runtime_api.rs # Runtime API server
│       ├── event_source/
│       │   ├── mod.rs
│       │   ├── sqs.rs
│       │   ├── kinesis.rs
│       │   └── dynamodb.rs
│       └── layers.rs          # Layer management
│
├── ruststack-auth/            # Authentication library
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── sigv4.rs           # Signature Version 4
│       ├── sigv2.rs           # Signature Version 2 (legacy)
│       └── presigned.rs       # Presigned URL validation
│
└── tests/                     # Integration tests
    ├── s3/
    ├── dynamodb/
    └── lambda/
```

---

## Component Design

### 1. HTTP Gateway

Built on **Axum** for its:
- Tower middleware ecosystem
- Excellent async performance
- Easy routing composition

```rust
// router.rs
pub fn create_router(services: ServiceRegistry) -> Router {
    Router::new()
        // S3 routes (both virtual-hosted and path-style)
        .route("/:bucket/*key", any(s3_handler))
        .route("/:bucket", any(s3_bucket_handler))
        // DynamoDB (single endpoint)
        .route("/dynamodb", post(dynamodb_handler))
        // Lambda
        .route("/lambda/*path", any(lambda_handler))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(AuthLayer::new())
                .layer(RequestIdLayer::new())
        )
}
```

### 2. S3 Service

Built on **s3s** framework with custom storage backend:

```rust
// ruststack-s3/src/service.rs
use s3s::{S3, S3Request, S3Response, S3Result};

pub struct RustStackS3 {
    storage: Arc<dyn ObjectStorage>,
    state: Arc<S3State>,
}

#[async_trait]
impl S3 for RustStackS3 {
    async fn get_object(
        &self,
        req: S3Request<GetObjectInput>,
    ) -> S3Result<S3Response<GetObjectOutput>> {
        let bucket = self.state.get_bucket(&req.input.bucket)?;
        let object = self.storage.get_object(&bucket, &req.input.key).await?;
        
        // Handle versioning, range requests, checksums...
        Ok(S3Response::new(output))
    }
    
    // ... other operations
}
```

**Storage Backend Trait:**

```rust
// ruststack-s3/src/storage/traits.rs
#[async_trait]
pub trait ObjectStorage: Send + Sync {
    async fn get_object(
        &self,
        bucket: &str,
        key: &str,
        version_id: Option<&str>,
    ) -> Result<StoredObject, StorageError>;
    
    async fn put_object(
        &self,
        bucket: &str,
        key: &str,
        body: impl AsyncRead + Send,
        metadata: ObjectMetadata,
    ) -> Result<PutObjectResult, StorageError>;
    
    async fn delete_object(
        &self,
        bucket: &str,
        key: &str,
        version_id: Option<&str>,
    ) -> Result<DeleteResult, StorageError>;
    
    async fn list_objects(
        &self,
        bucket: &str,
        prefix: Option<&str>,
        delimiter: Option<&str>,
        continuation_token: Option<&str>,
        max_keys: i32,
    ) -> Result<ListObjectsResult, StorageError>;
    
    // Multipart operations
    async fn create_multipart_upload(
        &self,
        bucket: &str,
        key: &str,
        metadata: ObjectMetadata,
    ) -> Result<String, StorageError>; // Returns upload ID
    
    async fn upload_part(
        &self,
        bucket: &str,
        key: &str,
        upload_id: &str,
        part_number: i32,
        body: impl AsyncRead + Send,
    ) -> Result<PartInfo, StorageError>;
    
    async fn complete_multipart_upload(
        &self,
        bucket: &str,
        key: &str,
        upload_id: &str,
        parts: Vec<CompletedPart>,
    ) -> Result<CompleteResult, StorageError>;
}
```

### 3. DynamoDB Service

**Phase 1: Proxy Mode (like LocalStack)**

```rust
// ruststack-dynamodb/src/proxy.rs
pub struct DynamoDBProxy {
    ddb_local: DynamoDBLocal,
    state: Arc<DynamoDBState>,
}

impl DynamoDBProxy {
    pub async fn handle_request(
        &self,
        action: &str,
        body: Bytes,
        context: &RequestContext,
    ) -> Result<Response, DynamoDBError> {
        // Pre-process request
        let modified_body = self.preprocess(action, body, context)?;
        
        // Forward to DynamoDB Local
        let response = self.ddb_local.forward(action, modified_body).await?;
        
        // Post-process response
        let final_response = self.postprocess(action, response, context)?;
        
        // Handle streams if needed
        if self.should_forward_to_stream(action) {
            self.forward_to_stream(action, &modified_body, context).await?;
        }
        
        Ok(final_response)
    }
}
```

**DynamoDB Local Management:**

```rust
// ruststack-dynamodb/src/server.rs
pub struct DynamoDBLocal {
    process: Option<Child>,
    port: u16,
    data_dir: PathBuf,
}

impl DynamoDBLocal {
    pub async fn start(&mut self) -> Result<(), DynamoDBError> {
        let jar_path = self.ensure_jar_exists().await?;
        
        self.process = Some(
            Command::new("java")
                .args([
                    "-Djava.library.path=./DynamoDBLocal_lib",
                    "-jar", &jar_path.display().to_string(),
                    "-port", &self.port.to_string(),
                    "-dbPath", &self.data_dir.display().to_string(),
                    "-sharedDb",
                ])
                .spawn()?
        );
        
        self.wait_for_ready().await
    }
    
    pub async fn forward(&self, action: &str, body: Bytes) -> Result<Bytes, DynamoDBError> {
        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://localhost:{}/", self.port))
            .header("X-Amz-Target", format!("DynamoDB_20120810.{}", action))
            .header("Content-Type", "application/x-amz-json-1.0")
            .body(body)
            .send()
            .await?;
        
        Ok(response.bytes().await?)
    }
}
```

### 4. Lambda Service

```rust
// ruststack-lambda/src/service.rs
pub struct LambdaService {
    functions: Arc<RwLock<HashMap<FunctionArn, Function>>>,
    container_manager: Arc<ContainerManager>,
    event_sources: Arc<EventSourceManager>,
}

impl LambdaService {
    pub async fn invoke(
        &self,
        function_name: &str,
        qualifier: Option<&str>,
        payload: Bytes,
        invocation_type: InvocationType,
    ) -> Result<InvocationResponse, LambdaError> {
        let function = self.resolve_function(function_name, qualifier)?;
        
        match invocation_type {
            InvocationType::RequestResponse => {
                self.invoke_sync(&function, payload).await
            }
            InvocationType::Event => {
                self.invoke_async(&function, payload).await
            }
            InvocationType::DryRun => {
                self.validate_invocation(&function, &payload)?;
                Ok(InvocationResponse::dry_run())
            }
        }
    }
    
    async fn invoke_sync(
        &self,
        function: &Function,
        payload: Bytes,
    ) -> Result<InvocationResponse, LambdaError> {
        let container = self.container_manager
            .acquire_container(&function.runtime, &function.handler)
            .await?;
        
        let result = container.invoke(payload).await?;
        
        self.container_manager.release_container(container).await;
        
        Ok(result)
    }
}
```

**Container Management:**

```rust
// ruststack-lambda/src/invocation/container.rs
pub struct ContainerManager {
    docker: Docker,
    warm_containers: DashMap<RuntimeKey, VecDeque<Container>>,
}

impl ContainerManager {
    pub async fn acquire_container(
        &self,
        runtime: &Runtime,
        handler: &str,
    ) -> Result<Container, LambdaError> {
        let key = RuntimeKey::new(runtime, handler);
        
        // Try to get warm container
        if let Some(container) = self.get_warm_container(&key) {
            return Ok(container);
        }
        
        // Create new container
        self.create_container(runtime, handler).await
    }
    
    async fn create_container(
        &self,
        runtime: &Runtime,
        handler: &str,
    ) -> Result<Container, LambdaError> {
        let image = self.get_runtime_image(runtime);
        
        let container = self.docker
            .create_container(
                Some(CreateContainerOptions { name: &uuid::Uuid::new_v4().to_string() }),
                Config {
                    image: Some(image),
                    env: Some(vec![
                        format!("_HANDLER={}", handler),
                        "AWS_LAMBDA_FUNCTION_NAME=test".into(),
                        "AWS_LAMBDA_FUNCTION_VERSION=$LATEST".into(),
                        // ... more env vars
                    ]),
                    exposed_ports: Some(hashmap! { "9001/tcp" => {} }),
                    host_config: Some(HostConfig {
                        port_bindings: Some(hashmap! {
                            "9001/tcp" => Some(vec![PortBinding {
                                host_ip: Some("0.0.0.0".into()),
                                host_port: Some("0".into()), // Dynamic port
                            }])
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }
            )
            .await?;
        
        self.docker.start_container(&container.id, None::<StartContainerOptions<String>>).await?;
        
        Ok(Container::new(container.id, self.get_container_port(&container.id).await?))
    }
}
```

**Runtime API:**

```rust
// ruststack-lambda/src/invocation/runtime_api.rs
pub fn runtime_api_router() -> Router {
    Router::new()
        .route(
            "/2018-06-01/runtime/invocation/next",
            get(get_next_invocation)
        )
        .route(
            "/2018-06-01/runtime/invocation/:request_id/response",
            post(post_response)
        )
        .route(
            "/2018-06-01/runtime/invocation/:request_id/error",
            post(post_error)
        )
        .route(
            "/2018-06-01/runtime/init/error",
            post(post_init_error)
        )
}

async fn get_next_invocation(
    State(state): State<RuntimeApiState>,
) -> impl IntoResponse {
    // Block until invocation available
    let invocation = state.invocation_queue.recv().await;
    
    (
        StatusCode::OK,
        [
            ("Lambda-Runtime-Aws-Request-Id", invocation.request_id),
            ("Lambda-Runtime-Invoked-Function-Arn", invocation.function_arn),
            ("Lambda-Runtime-Deadline-Ms", invocation.deadline_ms.to_string()),
        ],
        invocation.payload,
    )
}
```

---

## State Management

### Multi-Account/Region Support

```rust
// ruststack-core/src/account.rs
pub struct StateStore<T> {
    data: DashMap<AccountRegionKey, T>,
}

#[derive(Hash, Eq, PartialEq)]
pub struct AccountRegionKey {
    account_id: String,
    region: String,
}

impl<T: Default> StateStore<T> {
    pub fn get_or_create(&self, account_id: &str, region: &str) -> dashmap::mapref::one::Ref<AccountRegionKey, T> {
        let key = AccountRegionKey::new(account_id, region);
        self.data.entry(key).or_default()
    }
}
```

### Event Bus

For cross-service communication (e.g., S3 notifications to Lambda):

```rust
// ruststack-core/src/event.rs
pub enum ServiceEvent {
    S3ObjectCreated {
        bucket: String,
        key: String,
        size: u64,
        etag: String,
    },
    S3ObjectDeleted {
        bucket: String,
        key: String,
        version_id: Option<String>,
    },
    DynamoDBStreamRecord {
        table_name: String,
        event_name: String,
        keys: serde_json::Value,
        new_image: Option<serde_json::Value>,
        old_image: Option<serde_json::Value>,
    },
}

pub struct EventBus {
    subscribers: DashMap<EventType, Vec<Sender<ServiceEvent>>>,
}

impl EventBus {
    pub async fn publish(&self, event: ServiceEvent) {
        let event_type = event.event_type();
        if let Some(subscribers) = self.subscribers.get(&event_type) {
            for subscriber in subscribers.iter() {
                let _ = subscriber.send(event.clone()).await;
            }
        }
    }
    
    pub fn subscribe(&self, event_type: EventType) -> Receiver<ServiceEvent> {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        self.subscribers
            .entry(event_type)
            .or_default()
            .push(tx);
        rx
    }
}
```

---

## Configuration

```rust
// ruststack/src/config.rs
#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_port")]
    pub port: u16,
    
    #[serde(default)]
    pub services: ServicesConfig,
    
    #[serde(default)]
    pub storage: StorageConfig,
    
    #[serde(default)]
    pub persistence: PersistenceConfig,
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

fn default_port() -> u16 { 4566 }
fn default_true() -> bool { true }
```

---

## Dependency Selection

### Core Dependencies

| Dependency | Purpose | Version |
|------------|---------|---------|
| `tokio` | Async runtime | 1.x |
| `axum` | HTTP framework | 0.7 |
| `tower` | Middleware | 0.5 |
| `hyper` | HTTP primitives | 1.x |
| `serde` | Serialization | 1.x |
| `serde_json` | JSON | 1.x |
| `quick-xml` | XML for S3 | 0.31 |
| `thiserror` | Error handling | 2.x |
| `tracing` | Logging/tracing | 0.1 |
| `dashmap` | Concurrent maps | 6.x |
| `bytes` | Byte buffers | 1.x |

### Service-Specific

| Dependency | Service | Purpose |
|------------|---------|---------|
| `s3s` | S3 | S3 API framework |
| `bollard` | Lambda | Docker API |
| `md-5`, `sha1`, `sha2` | S3 | Checksums |
| `crc32fast` | S3 | CRC checksums |
| `hmac` | Auth | HMAC for SigV4 |
| `uuid` | All | Request IDs |
| `chrono` | All | Timestamps |

### Optional Dependencies

| Dependency | Feature | Purpose |
|------------|---------|---------|
| `reqwest` | DynamoDB | HTTP client for proxy |
| `tokio-rustls` | TLS | HTTPS support |
| `aws-config` | Testing | AWS SDK config |
| `aws-sdk-s3` | Testing | S3 integration tests |

---

## Performance Considerations

### Memory Management

1. **Streaming Bodies**: Never buffer entire objects in memory
2. **Object Pools**: Reuse buffers and connections
3. **Lazy Loading**: Load state on demand

### Concurrency

1. **Lock-Free Data Structures**: Use `DashMap` for concurrent access
2. **Connection Pooling**: For DynamoDB Local proxy
3. **Container Pool**: Keep warm Lambda containers

### I/O Optimization

1. **Zero-Copy**: Use `Bytes` for buffer management
2. **Vectored I/O**: For multipart writes
3. **Async File I/O**: Use `tokio::fs`

---

## Security Considerations

### Request Validation

- Validate all input parameters
- Enforce bucket/key name rules
- Rate limiting (configurable)

### Authentication

- Full SigV4 validation (optional, can be disabled for local dev)
- Credential checking (optional)

### Isolation

- Per-account state isolation
- Container isolation for Lambda

---

## Future Extensibility

### Additional Services (Post-MVP)

- SQS (queue between services)
- SNS (notifications)
- KMS (encryption keys)
- CloudWatch Logs (Lambda logs)
- EventBridge (event routing)

### Storage Backends

- S3-compatible remote storage
- Distributed storage (for multi-instance)

### Deployment Modes

- Single binary (current)
- Docker container
- Kubernetes operator
