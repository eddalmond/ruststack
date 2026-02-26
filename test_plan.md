# RustStack Test Plan

## Overview

This document outlines the test coverage strategy for the RustStack project - a Rust-based AWS service emulator. The project consists of multiple crates that mock various AWS services for local development and testing.

## Current Test Coverage

### Summary

| Package | Tests | Status |
|---------|-------|--------|
| ruststack-apigateway | 3 | ✅ Basic |
| ruststack-auth | 2 | ✅ Basic |
| ruststack-cognito | 25 | ✅ Excellent |
| ruststack-core | 6 | ✅ Good |
| ruststack-dynamodb | ~40+ | ✅ Excellent |
| ruststack-firehose | 16 | ✅ Good |
| ruststack-iam | ~5 | ✅ Basic |
| ruststack-lambda | ~10 | ✅ Good |
| ruststack-s3 | ~15 | ✅ Good |
| ruststack-secretsmanager | 14 | ✅ Good |
| ruststack-sns | 26 | ✅ Excellent |
| ruststack-sqs | 23 | ✅ Excellent |

**Total: ~185 tests across 12 packages**

## Package-by-Package Test Strategy

### 1. ruststack-cognito ✅ (Priority: LOW)

Cognito handles user authentication and pool management.

**Current:** 25 tests (excellent)

Tests cover:
- User pool creation with valid region/name
- User creation flow
- UserStatus enum variants - serialization/deserialization
- JWT token generation and validation
- Authentication flow (initiate_auth, respond_to_auth_challenge)
- User attribute handling
- Enable/disable user

**Test File Location:** `ruststack-cognito/src/storage.rs`

### 2. ruststack-sns ✅ (Priority: LOW)

SNS handles pub/sub messaging and topic management.

**Current:** 26 tests (excellent)

Tests cover:
- SnsState initialization
- Topic creation
- Subscription enum variants - subscription handling
- Message publishing to topics
- Fan-out to SQS
- HTTP/Lambda/Email subscription protocols

**Test File Location:** `ruststack-sns/src/storage.rs`

### 3. ruststack-sqs ✅ (Priority: LOW)

SQS handles queue management and message processing.

**Current:** 23 tests (excellent)

Tests cover:
- Queue creation
- Queue deletion
- Message sending
- Message retrieval
- Message deletion
- Dead letter queue handling
- FIFO vs standard queue differences
- Message batching
- Approximate receive count

**Test File Location:** `ruststack-sqs/src/storage.rs`

### 4. ruststack-secretsmanager ✅ (Priority: LOW)

Secrets Manager handles secure parameter storage.

**Current:** 14 tests (good)

Tests cover:
- Secret creation with various data types (string, binary)
- Secret versioning and rotation
- GetSecretValue flow with version stages
- Delete secret (scheduled vs force)
- Describe secret
- List secrets
- Secret tags

**Test File Location:** `ruststack-secretsmanager/src/storage.rs`

### 5. ruststack-firehose ✅ (Priority: LOW)

Firehose handles streaming data to destinations.

**Current:** 16 tests (good)

Tests cover:
- Delivery stream creation
- Record batching
- S3 destination configuration
- Buffering configuration
- Describe/List/Delete delivery streams
- Error handling for nonexistent streams

**Test File Location:** `ruststack-firehose/src/storage.rs`

### 6. ruststack-apigateway ✅ (Priority: LOW)

API Gateway handles REST API management.

**Current:** 3 tests (basic)

**Recommended Additions:**
- Route parameter parsing
- Integration response mapping
- Request/response transformation
- API key validation

### 7. ruststack-auth ✅ (Priority: LOW)

Auth handles AWS signature verification.

**Current:** 2 tests (basic)

**Recommended Additions:**
- More SigV4 variants
- Date parsing edge cases
- Header validation

### 8. ruststack-core ✅ (Priority: LOW)

Core contains shared utilities.

**Current:** 6 tests (good)

**Recommended Additions:**
- Error chain handling
- Region parsing

### 9. ruststack-iam ✅ (Priority: LOW)

IAM handles access control.

**Current:** ~5 tests (basic)

**Recommended Additions:**
- Policy document parsing
- Role assumption

### 10. ruststack-lambda ✅ (Priority: LOW)

Lambda handles function execution.

**Current:** ~10 tests (good)

**Recommended Additions:**
- Cold start simulation
- Environment variable handling

### 11. ruststack-s3 ✅ (Priority: LOW)

S3 handles object storage.

**Current:** ~15 tests (good)

**Recommended Additions:**
- Multipart upload completion
- Bucket policy enforcement
- CORS configuration

### 12. ruststack-dynamodb ✅ (Priority: LOW)

DynamoDB handles NoSQL data storage.

**Current:** ~40+ tests (excellent)

**Recommended Additions:**
- GSI/LSI operations
- TTL handling
- Cursor pagination

## Test Types Strategy

### Unit Tests
- Target: Core business logic in each crate
- Location: `src/<module>.rs` with `#[cfg(test)] mod tests`
- Run with: `cargo test --lib`

### Integration Tests
- Target: Cross-crate workflows
- Location: `tests/integration/`
- Examples:
  - S3 → Lambda invocation
  - SNS → SQS fan-out
  - Cognito → IAM role assumption

### Property Tests
- Target: Data transformation and serialization
- Crates: dynamodb, s3 (already has some)
- Use: `proptest` or `quickcheck`

## Testing Patterns

### Common Test Utilities

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    // Helper to create test state
    fn test_state() -> SnsState {
        SnsState::new()
    }
    
    // Helper to create test messages
    fn test_message() -> String {
        r#"{"test": "data"}"#.to_string()
    }
}
```

### Async Testing

For async operations:

```rust
#[tokio::test]
async fn test_async_operation() {
    // Test async code with tokio
}
```

### Snapshot Testing

For complex responses:

```rust
#[test]
fn test_response_format() {
    let response = generate_response();
    assert_json_snapshot!(response);
}
```

## CI Integration

### GitHub Actions

```yaml
- name: Run tests
  run: cargo test --workspace --all-features

- name: Run clippy
  run: cargo clippy --workspace -- -D warnings

- name: Run fmt
  run: cargo fmt --check
```

### Coverage

Consider adding `cargo-llvm-cov` for coverage reports:

```yaml
- name: Coverage
  run: cargo llvm-cov --workspace --lcov --output-path lcov.info
```

## Priority Roadmap

### Phase 1: Critical Services (COMPLETED)
1. **ruststack-sqs** - 23 tests ✅
   - Queue CRUD operations
   - Message lifecycle
   - Batching

2. **ruststack-sns** - 26 tests ✅
   - Topic management
   - Subscription handling
   - Message publishing

3. **ruststack-cognito** - 25 tests ✅
   - User pool management
   - Authentication flows
   - Token handling

### Phase 2: Supporting Services (COMPLETED)
4. **ruststack-secretsmanager** - 14 tests ✅
5. **ruststack-firehose** - 16 tests ✅

### Phase 3: Enhancements (Ongoing)
6. Add property-based tests
7. Add integration tests across crates
8. Improve error handling coverage

## Running Tests

```bash
# All tests
cargo test --workspace

# Single crate
cargo test -p ruststack-sqs

# With coverage
cargo llvm-cov --workspace

# Watch mode (for development)
cargo watch -x test
```

## References

- [Rust Testing Book](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [tokio testing](https://tokio.rs/tokio/topics/testing)
- [proptest](https://docs.rs/proptest/)
- [rstest](https://docs.rs/rstest/)
