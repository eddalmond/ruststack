# RustStack Implementation Plan (Focused Scope)

## Overview

RustStack is a Rust-based AWS local emulator for **integration testing Flask/Lambda applications**. This plan prioritizes depth over breadth - bulletproof core operations.

**Target Timeline:** 8 weeks for production-ready MVP

---

## Scope Definition

### In Scope (Must Be Bulletproof)

**S3:**
- GetObject (with range requests)
- PutObject (streaming, Content-MD5)
- DeleteObject
- HeadObject
- ListObjectsV2 (pagination, prefix)
- CreateBucket, DeleteBucket, HeadBucket
- Exact AWS error codes

**DynamoDB:**
- GetItem, PutItem, DeleteItem, UpdateItem
- Query, Scan (with expressions)
- CreateTable, DeleteTable, DescribeTable
- GSI support
- Condition expression failures

**Lambda:**
- CreateFunction (zip upload)
- Invoke (sync, API Gateway v1 format)
- DeleteFunction
- Flask/WSGI handler execution

### Explicitly Out of Scope

- S3: versioning, lifecycle, replication, multipart, notifications
- DynamoDB: streams, transactions, DAX, global tables
- Lambda: layers, provisioned concurrency, event sources, aliases

---

## Phase 1: Project Setup & S3 Core (Weeks 1-2)

### Week 1: Foundation

#### Day 1-2: Project Setup
- [x] Initialize workspace structure
- [x] Create core crates (ruststack-core, ruststack-auth, etc.)
- [x] Set up error types and request ID generation
- [ ] Configure CI/CD (GitHub Actions)
- [ ] Add rustfmt, clippy, test jobs

#### Day 3-5: S3 Storage Backend
- [x] Define `ObjectStorage` trait
- [x] Implement `EphemeralStorage` with DashMap
- [x] Add unit tests for storage operations
- [ ] Benchmark storage performance

**Deliverable:** Storage backend with full test coverage

### Week 2: S3 HTTP Layer

#### Day 1-3: Integrate s3s Framework
- [ ] Add s3s dependency
- [ ] Implement `S3` trait for `RustStackS3`
- [ ] Wire up HTTP routing
- [ ] Test with aws-sdk-s3

#### Day 4-5: Error Handling
- [ ] Implement S3-specific errors (NoSuchKey, NoSuchBucket, etc.)
- [ ] XML error response formatting
- [ ] Add error code tests against AWS behavior

**Deliverable:** Working S3 with GetObject, PutObject, DeleteObject, HeadObject, ListObjectsV2

### Milestone 1 Tests

```rust
// All must pass against RustStack AND AWS
#[tokio::test] async fn test_put_get_object();
#[tokio::test] async fn test_get_nonexistent_key_returns_no_such_key();
#[tokio::test] async fn test_delete_nonexistent_bucket_returns_no_such_bucket();
#[tokio::test] async fn test_list_objects_pagination();
#[tokio::test] async fn test_head_object_metadata();
#[tokio::test] async fn test_range_request();
```

---

## Phase 2: DynamoDB (Weeks 3-4)

### Week 3: DynamoDB Local Integration

#### Day 1-2: Server Management
- [x] DynamoDB Local download/extraction
- [x] Process lifecycle management
- [ ] Health check implementation
- [ ] Automatic port allocation

#### Day 3-5: Proxy Implementation
- [x] Basic request forwarding
- [ ] ARN transformation (ddblocal → proper ARN)
- [ ] Error response passthrough
- [ ] Request/response logging

**Deliverable:** DynamoDB proxy with CreateTable, DeleteTable, DescribeTable

### Week 4: DynamoDB Operations

#### Day 1-3: Item Operations
- [ ] GetItem, PutItem, DeleteItem, UpdateItem
- [ ] Verify condition expression failures
- [ ] Test with real update expressions

#### Day 4-5: Query/Scan
- [ ] Query with key conditions
- [ ] GSI queries
- [ ] Scan with filter expressions
- [ ] Pagination (LastEvaluatedKey)

**Deliverable:** Full DynamoDB item operations with expression support

### Milestone 2 Tests

```rust
#[tokio::test] async fn test_put_get_item();
#[tokio::test] async fn test_condition_expression_fails();
#[tokio::test] async fn test_update_expression();
#[tokio::test] async fn test_query_with_key_condition();
#[tokio::test] async fn test_query_gsi();
#[tokio::test] async fn test_scan_with_filter();
#[tokio::test] async fn test_query_pagination();
```

---

## Phase 3: Lambda (Weeks 5-6)

### Week 5: Container Execution

#### Day 1-2: Docker Integration
- [ ] Bollard Docker client setup
- [ ] Runtime image pulling (Python 3.11/3.12)
- [ ] Container creation with mounts

#### Day 3-5: Runtime API
- [x] Runtime API server (axum routes)
- [ ] Invocation delivery
- [ ] Response collection
- [ ] Timeout handling

**Deliverable:** Basic Lambda invocation working

### Week 6: Flask Compatibility

#### Day 1-3: API Gateway Event Format
- [x] API Gateway v1 event structure
- [x] Response format
- [ ] HTTP method/path/headers mapping
- [ ] Query string handling

#### Day 4-5: Integration Testing
- [ ] Create test Flask app
- [ ] Test with Mangum adapter
- [ ] Verify request/response cycle
- [ ] Environment variable injection

**Deliverable:** Flask app running in Lambda with correct behavior

### Milestone 3 Tests

```rust
#[tokio::test] async fn test_create_invoke_delete_function();
#[tokio::test] async fn test_flask_get_request();
#[tokio::test] async fn test_flask_post_with_body();
#[tokio::test] async fn test_flask_error_response();
#[tokio::test] async fn test_environment_variables();
```

---

## Phase 4: Integration & Polish (Weeks 7-8)

### Week 7: End-to-End Testing

#### Day 1-3: Full Stack Tests
- [ ] Flask app using S3 for file storage
- [ ] Flask app using DynamoDB for data
- [ ] Error scenarios (missing objects, failed conditions)

#### Day 4-5: Performance & Reliability
- [ ] Cold start benchmarks
- [ ] Concurrent request handling
- [ ] Memory usage profiling
- [ ] Stress testing

### Week 8: Documentation & Release

#### Day 1-3: Documentation
- [ ] README with quickstart
- [ ] Configuration reference
- [ ] Troubleshooting guide
- [ ] API compatibility matrix

#### Day 4-5: Release
- [ ] Docker image (multi-arch)
- [ ] Binary releases (Linux, macOS)
- [ ] GitHub release
- [ ] Example project

---

## Test Strategy

### Unit Tests (Per Crate)

Each crate has `src/` code and `tests/` for unit tests:

```
ruststack-s3/
├── src/
│   ├── storage/
│   │   ├── ephemeral.rs    # Has inline #[cfg(test)] mod tests
│   │   └── traits.rs
│   └── service.rs
└── tests/
    └── storage_tests.rs    # Additional integration tests
```

### Integration Tests

Use AWS SDK for Rust against RustStack:

```rust
// tests/integration/s3.rs
async fn create_client() -> aws_sdk_s3::Client {
    let config = aws_config::defaults(BehaviorVersion::latest())
        .endpoint_url("http://localhost:4566")
        .credentials_provider(Credentials::new("test", "test", None, None, "test"))
        .load()
        .await;
    aws_sdk_s3::Client::new(&config)
}
```

### Compatibility Tests

Run same tests against LocalStack and AWS to verify behavior matches:

```bash
# Run against RustStack
ENDPOINT_URL=http://localhost:4566 cargo test --test compatibility

# Run against LocalStack (reference)
ENDPOINT_URL=http://localhost:4567 cargo test --test compatibility

# Run against AWS (golden master, be careful with costs)
AWS_PROFILE=test cargo test --test compatibility -- --ignored
```

---

## Success Criteria

### MVP Definition

RustStack MVP is complete when:

1. **S3:** All P0 operations work with correct error codes
2. **DynamoDB:** All P0 operations work, including expression failures
3. **Lambda:** Flask app can be invoked with correct API Gateway event format
4. **Integration:** A sample Flask app using S3 + DynamoDB passes all tests

### Metrics

| Metric | Target |
|--------|--------|
| S3 operation coverage | 5 operations (100% of scope) |
| DynamoDB operation coverage | 8 operations (100% of scope) |
| Lambda operation coverage | 3 operations (100% of scope) |
| Error code accuracy | 100% match with AWS |
| Cold start time | < 500ms |
| Test pass rate | 100% |

---

## Risk Mitigation

### High Risk: DynamoDB Local Compatibility

**Risk:** DynamoDB Local may have subtle differences from AWS.

**Mitigation:**
- Run compatibility tests against both
- Document known differences
- Have workaround layer for critical issues

### Medium Risk: Lambda Container Startup

**Risk:** Container cold start too slow for test iteration.

**Mitigation:**
- Keep containers warm between invocations
- Pre-pull runtime images
- Consider hot-reload for development

### Low Risk: s3s Framework Limitations

**Risk:** s3s may not expose all needed customization.

**Mitigation:**
- Fork if necessary
- Most S3 behavior is standard HTTP

---

## Dependencies

### Runtime Dependencies

| Dependency | Purpose | Notes |
|------------|---------|-------|
| tokio | Async runtime | Required |
| axum | HTTP server | |
| s3s | S3 framework | Core S3 impl |
| bollard | Docker API | Lambda containers |
| DynamoDB Local | DynamoDB engine | Java JAR |

### Development Dependencies

| Dependency | Purpose |
|------------|---------|
| aws-sdk-s3 | Testing |
| aws-sdk-dynamodb | Testing |
| aws-sdk-lambda | Testing |

### External Dependencies

| Dependency | How to Obtain |
|------------|---------------|
| Docker | Must be installed |
| Java 11+ | For DynamoDB Local |
| DynamoDB Local JAR | Download from AWS |

---

## File Structure (Final)

```
ruststack/
├── Cargo.toml
├── README.md
├── ruststack/                 # Main binary
│   └── src/main.rs
├── ruststack-core/            # Shared types
│   └── src/{error,account,request_id}.rs
├── ruststack-auth/            # Auth (minimal for local)
│   └── src/sigv4.rs
├── ruststack-s3/              # S3 service
│   └── src/{service,storage/*}.rs
├── ruststack-dynamodb/        # DynamoDB proxy
│   └── src/{proxy,server}.rs
├── ruststack-lambda/          # Lambda execution
│   └── src/{service,function,invocation,runtime_api}.rs
├── tests/
│   ├── s3/
│   ├── dynamodb/
│   └── lambda/
└── examples/
    └── flask-app/             # Sample Flask app for testing
```

---

## Next Steps

1. **Immediate:** Set up CI/CD with GitHub Actions
2. **This week:** Complete S3 integration with s3s
3. **Next week:** DynamoDB Local integration and proxy
4. **Week 5:** Lambda container execution

The focused scope means we can deliver a production-quality tool in 8 weeks rather than a broad but unreliable one in 16 weeks.
