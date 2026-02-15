# RustStack Implementation Plan

## Overview

This document outlines the implementation plan for RustStack, a Rust-based AWS local emulator. The plan is divided into phases with clear milestones and deliverables.

**Target Timeline:** 12-16 weeks for MVP

---

## Phase 0: Project Setup (Week 1)

### Goals
- Set up project structure and tooling
- Establish CI/CD pipeline
- Create development environment

### Tasks

- [ ] **0.1** Initialize Cargo workspace
- [ ] **0.2** Set up project structure per ARCHITECTURE.md
- [ ] **0.3** Configure CI (GitHub Actions)
  - Lint (clippy)
  - Format (rustfmt)
  - Build (debug + release)
  - Unit tests
- [ ] **0.4** Add pre-commit hooks
- [ ] **0.5** Create Dockerfile for development
- [ ] **0.6** Set up integration test infrastructure
- [ ] **0.7** Write README with quick start

### Deliverables
- Working `cargo build` and `cargo test`
- CI passing on all commits
- Development Docker image

---

## Phase 1: Core Infrastructure (Weeks 2-3)

### Goals
- HTTP server with routing
- Request authentication framework
- Error handling and response formatting

### Tasks

- [ ] **1.1** Create `ruststack-core` crate
  - [ ] AWS error types (S3Error, DynamoDBError, LambdaError)
  - [ ] Request context (account_id, region)
  - [ ] Request ID generation
  - [ ] Response formatting utilities

- [ ] **1.2** Create `ruststack-auth` crate
  - [ ] SigV4 signature validation (can disable for dev)
  - [ ] SigV2 signature validation (legacy)
  - [ ] Presigned URL validation
  - [ ] Credential extraction

- [ ] **1.3** Create main `ruststack` binary crate
  - [ ] CLI argument parsing (clap)
  - [ ] Configuration loading (TOML/env)
  - [ ] Axum server setup
  - [ ] Basic routing skeleton

- [ ] **1.4** Implement request logging
  - [ ] Request/response tracing
  - [ ] Performance metrics
  - [ ] Configurable log levels

### Deliverables
- HTTP server accepting requests on port 4566
- Authentication middleware (bypassable)
- Structured error responses
- Request logging

### Tests
- Unit tests for auth module
- Integration test: server starts and responds

---

## Phase 2: S3 Core (Weeks 4-7)

### Goals
- Core S3 object operations
- In-memory storage backend
- AWS SDK compatibility

### Week 4-5: Basic Operations

- [ ] **2.1** Integrate s3s framework
  - [ ] Configure s3s with custom backend
  - [ ] Set up routing for S3 endpoints
  - [ ] Virtual-hosted style support
  - [ ] Path-style support

- [ ] **2.2** Implement storage backend trait
  - [ ] Define `ObjectStorage` trait
  - [ ] Implement `EphemeralStorage`
  - [ ] Bucket management (create, delete, list)
  - [ ] Object metadata storage

- [ ] **2.3** Implement bucket operations
  - [ ] CreateBucket
  - [ ] DeleteBucket
  - [ ] HeadBucket
  - [ ] ListBuckets
  - [ ] GetBucketLocation

- [ ] **2.4** Implement object CRUD
  - [ ] PutObject (streaming body)
  - [ ] GetObject (with range support)
  - [ ] HeadObject
  - [ ] DeleteObject
  - [ ] DeleteObjects (batch)
  - [ ] CopyObject

### Week 6: Multipart Upload

- [ ] **2.5** Implement multipart upload
  - [ ] CreateMultipartUpload
  - [ ] UploadPart
  - [ ] UploadPartCopy
  - [ ] CompleteMultipartUpload
  - [ ] AbortMultipartUpload
  - [ ] ListParts
  - [ ] ListMultipartUploads

### Week 7: Versioning & Metadata

- [ ] **2.6** Implement versioning
  - [ ] PutBucketVersioning
  - [ ] GetBucketVersioning
  - [ ] Versioned storage backend
  - [ ] Delete markers
  - [ ] ListObjectVersions

- [ ] **2.7** Implement metadata operations
  - [ ] GetObjectTagging
  - [ ] PutObjectTagging
  - [ ] DeleteObjectTagging
  - [ ] GetBucketTagging
  - [ ] PutBucketTagging

- [ ] **2.8** Implement checksums
  - [ ] MD5 (ETag)
  - [ ] Content-MD5 validation
  - [ ] CRC32, CRC32C
  - [ ] SHA1, SHA256

### Deliverables
- Functional S3 service with core operations
- Multipart upload support
- Versioning support
- Checksum validation

### Tests
- Unit tests for storage backend
- Integration tests with aws-sdk-s3
- Multipart upload tests
- Versioning behavior tests

### Compatibility Tests (Priority Operations)

```rust
// tests/s3/basic.rs
#[tokio::test]
async fn test_put_get_object() {
    let client = create_test_client().await;
    
    // Put object
    client.put_object()
        .bucket("test-bucket")
        .key("test-key")
        .body(ByteStream::from_static(b"hello world"))
        .send()
        .await
        .unwrap();
    
    // Get object
    let result = client.get_object()
        .bucket("test-bucket")
        .key("test-key")
        .send()
        .await
        .unwrap();
    
    let body = result.body.collect().await.unwrap().into_bytes();
    assert_eq!(&body[..], b"hello world");
}
```

---

## Phase 3: DynamoDB (Weeks 8-10)

### Goals
- DynamoDB Local integration
- Full API proxy
- Streams support

### Week 8: DynamoDB Local Setup

- [ ] **3.1** Create `ruststack-dynamodb` crate
  - [ ] DynamoDB Local download/management
  - [ ] Process lifecycle management
  - [ ] Health checking

- [ ] **3.2** Implement proxy layer
  - [ ] Request forwarding
  - [ ] ARN transformation
  - [ ] Error mapping

- [ ] **3.3** Add basic API routing
  - [ ] CreateTable
  - [ ] DeleteTable
  - [ ] DescribeTable
  - [ ] ListTables

### Week 9: Core Operations

- [ ] **3.4** Implement item operations
  - [ ] PutItem
  - [ ] GetItem
  - [ ] UpdateItem
  - [ ] DeleteItem
  - [ ] Query
  - [ ] Scan

- [ ] **3.5** Implement batch operations
  - [ ] BatchWriteItem
  - [ ] BatchGetItem
  - [ ] TransactWriteItems
  - [ ] TransactGetItems

### Week 10: Advanced Features

- [ ] **3.6** Implement streams support
  - [ ] Stream record generation
  - [ ] DescribeStream
  - [ ] GetShardIterator
  - [ ] GetRecords

- [ ] **3.7** Implement tagging and TTL
  - [ ] TagResource
  - [ ] UntagResource
  - [ ] ListTagsOfResource
  - [ ] UpdateTimeToLive
  - [ ] DescribeTimeToLive

### Deliverables
- Full DynamoDB API via proxy
- Streams support
- State persistence

### Tests
- Table creation/deletion
- CRUD operations
- Batch operations
- Transaction tests

---

## Phase 4: Lambda (Weeks 11-14)

### Goals
- Container-based Lambda execution
- Runtime API implementation
- Event source mappings

### Week 11: Function Management

- [ ] **4.1** Create `ruststack-lambda` crate
  - [ ] Function models
  - [ ] State management
  - [ ] ARN parsing/validation

- [ ] **4.2** Implement function CRUD
  - [ ] CreateFunction
  - [ ] GetFunction
  - [ ] UpdateFunctionCode
  - [ ] UpdateFunctionConfiguration
  - [ ] DeleteFunction
  - [ ] ListFunctions

- [ ] **4.3** Implement versioning
  - [ ] PublishVersion
  - [ ] ListVersionsByFunction
  - [ ] GetFunctionConfiguration

### Week 12: Container Execution

- [ ] **4.4** Implement container management
  - [ ] Docker integration (bollard)
  - [ ] Runtime image mapping
  - [ ] Container pool management
  - [ ] Warm container reuse

- [ ] **4.5** Implement Runtime API
  - [ ] /runtime/invocation/next
  - [ ] /runtime/invocation/{id}/response
  - [ ] /runtime/invocation/{id}/error
  - [ ] /runtime/init/error

- [ ] **4.6** Implement Invoke
  - [ ] Synchronous invocation
  - [ ] Asynchronous invocation
  - [ ] DryRun validation

### Week 13: Aliases & Permissions

- [ ] **4.7** Implement aliases
  - [ ] CreateAlias
  - [ ] GetAlias
  - [ ] UpdateAlias
  - [ ] DeleteAlias
  - [ ] ListAliases
  - [ ] Weighted routing

- [ ] **4.8** Implement permissions
  - [ ] AddPermission
  - [ ] RemovePermission
  - [ ] GetPolicy

### Week 14: Event Source Mappings

- [ ] **4.9** Implement event source framework
  - [ ] CreateEventSourceMapping
  - [ ] GetEventSourceMapping
  - [ ] UpdateEventSourceMapping
  - [ ] DeleteEventSourceMapping
  - [ ] ListEventSourceMappings

- [ ] **4.10** Implement SQS integration
  - [ ] SQS event polling
  - [ ] Batch processing
  - [ ] Error handling

### Deliverables
- Lambda function execution
- Docker-based runtime
- Basic event source mappings

### Tests
- Function creation/execution
- Multiple runtimes (Node.js, Python)
- Alias routing
- Event source processing

---

## Phase 5: Integration & Polish (Weeks 15-16)

### Goals
- Cross-service integration
- Performance optimization
- Documentation

### Week 15: Integration

- [ ] **5.1** S3 to Lambda notifications
  - [ ] Event filtering
  - [ ] Lambda invocation on S3 events

- [ ] **5.2** DynamoDB Streams to Lambda
  - [ ] Stream processing
  - [ ] Batch windowing

- [ ] **5.3** End-to-end testing
  - [ ] Multi-service workflows
  - [ ] Error scenarios
  - [ ] Edge cases

### Week 16: Polish

- [ ] **5.4** Performance optimization
  - [ ] Profiling
  - [ ] Memory optimization
  - [ ] Startup time reduction

- [ ] **5.5** Documentation
  - [ ] API documentation
  - [ ] Usage examples
  - [ ] Configuration reference
  - [ ] Troubleshooting guide

- [ ] **5.6** Release preparation
  - [ ] Version tagging
  - [ ] Changelog
  - [ ] Docker hub image
  - [ ] Homebrew formula

### Deliverables
- Production-ready MVP
- Complete documentation
- Published releases

---

## Test Strategy

### Unit Tests

Each crate has its own unit tests:

```
ruststack-core/src/error.rs      → ruststack-core/tests/error_tests.rs
ruststack-s3/src/versioning.rs   → ruststack-s3/tests/versioning_tests.rs
ruststack-auth/src/sigv4.rs      → ruststack-auth/tests/sigv4_tests.rs
```

### Integration Tests

Using AWS SDK for Rust:

```rust
// tests/integration/s3_basic.rs
use aws_config::BehaviorVersion;
use aws_sdk_s3::Client;

async fn create_client() -> Client {
    let config = aws_config::defaults(BehaviorVersion::latest())
        .endpoint_url("http://localhost:4566")
        .load()
        .await;
    Client::new(&config)
}

#[tokio::test]
async fn test_bucket_lifecycle() {
    let client = create_client().await;
    
    // Create bucket
    client.create_bucket()
        .bucket("test-bucket")
        .send()
        .await
        .unwrap();
    
    // Verify exists
    let result = client.head_bucket()
        .bucket("test-bucket")
        .send()
        .await;
    assert!(result.is_ok());
    
    // Delete bucket
    client.delete_bucket()
        .bucket("test-bucket")
        .send()
        .await
        .unwrap();
}
```

### Snapshot Tests

Compare responses with known-good outputs:

```rust
#[tokio::test]
async fn test_error_response_format() {
    let client = create_client().await;
    
    let result = client.get_object()
        .bucket("nonexistent-bucket")
        .key("key")
        .send()
        .await;
    
    let error = result.unwrap_err();
    insta::assert_snapshot!(format!("{:?}", error));
}
```

### Compatibility Tests

Run same tests against LocalStack and AWS:

```bash
# Run against RustStack
ENDPOINT_URL=http://localhost:4566 cargo test --test compatibility

# Run against LocalStack
ENDPOINT_URL=http://localhost:4566 cargo test --test compatibility

# Run against AWS (careful!)
AWS_PROFILE=test cargo test --test compatibility
```

---

## Risk Assessment

### High Risk

| Risk | Mitigation |
|------|------------|
| Lambda cold start performance | Container pool, optimization |
| DynamoDB Local compatibility | Extensive testing, version pinning |
| S3 edge cases | Use s3s framework, comprehensive tests |

### Medium Risk

| Risk | Mitigation |
|------|------------|
| SigV4 implementation complexity | Use existing crypto crates |
| Cross-platform Docker issues | CI testing on Linux/macOS/Windows |
| Memory usage with large objects | Streaming, configurable limits |

### Low Risk

| Risk | Mitigation |
|------|------------|
| Rust async complexity | Well-established patterns |
| JSON/XML serialization | Use serde ecosystem |

---

## Success Criteria

### MVP (End of Phase 5)

1. **S3**: Pass 90%+ of basic operation tests
2. **DynamoDB**: All core CRUD operations working
3. **Lambda**: Sync invocation with Node.js/Python runtimes
4. **Performance**: < 100ms cold start for S3/DynamoDB
5. **Documentation**: Complete user guide

### Metrics

| Metric | Target |
|--------|--------|
| Test coverage | > 80% |
| S3 API coverage | 50+ operations |
| DynamoDB API coverage | 30+ operations |
| Lambda runtimes | 3+ (Node, Python, Go) |
| Startup time | < 2 seconds |
| Memory usage | < 100MB idle |

---

## Post-MVP Roadmap

### Phase 6: Additional Services
- SQS (message queuing)
- SNS (notifications)
- EventBridge (event routing)
- CloudWatch Logs

### Phase 7: Advanced Features
- Native DynamoDB engine (replace DDB Local)
- Distributed mode
- Kubernetes operator
- Web UI dashboard

### Phase 8: Ecosystem
- Terraform provider
- CDK support
- VS Code extension
- CLI tool

---

## Appendix: Priority API Operations

### S3 (MVP - 50 operations)

**Must Have:**
- CreateBucket, DeleteBucket, HeadBucket, ListBuckets
- PutObject, GetObject, HeadObject, DeleteObject, DeleteObjects
- CopyObject
- CreateMultipartUpload, UploadPart, CompleteMultipartUpload, AbortMultipartUpload
- ListObjects, ListObjectsV2

**Should Have:**
- PutBucketVersioning, GetBucketVersioning
- ListObjectVersions
- PutObjectTagging, GetObjectTagging
- PutBucketTagging, GetBucketTagging

**Nice to Have:**
- GetObjectAttributes
- PutBucketCors, GetBucketCors
- PutBucketLifecycle, GetBucketLifecycle

### DynamoDB (MVP - 30 operations)

**Must Have:**
- CreateTable, DeleteTable, DescribeTable, ListTables
- PutItem, GetItem, UpdateItem, DeleteItem
- Query, Scan
- BatchWriteItem, BatchGetItem

**Should Have:**
- TransactWriteItems, TransactGetItems
- UpdateTable
- TagResource, UntagResource

**Nice to Have:**
- DescribeTimeToLive, UpdateTimeToLive
- CreateGlobalTable (v2019)

### Lambda (MVP - 30 operations)

**Must Have:**
- CreateFunction, GetFunction, DeleteFunction
- UpdateFunctionCode, UpdateFunctionConfiguration
- Invoke
- ListFunctions

**Should Have:**
- PublishVersion, ListVersionsByFunction
- CreateAlias, GetAlias, DeleteAlias
- AddPermission, GetPolicy

**Nice to Have:**
- CreateEventSourceMapping
- PutFunctionConcurrency
- PublishLayerVersion
