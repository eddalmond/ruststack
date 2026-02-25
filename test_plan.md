# RustStack Test Plan

## Overview

This document outlines the test coverage strategy for the RustStack project - a Rust-based AWS service emulator. The project consists of multiple crates that mock various AWS services for local development and testing.

## Current Test Coverage

### Summary

| Package | Tests | Status |
|---------|-------|--------|
| ruststack-apigateway | 3 | ✅ Basic |
| ruststack-auth | 2 | ✅ Basic |
| ruststack-cognito | 0 | ❌ None |
| ruststack-core | 6 | ✅ Good |
| ruststack-dynamodb | ~40+ | ✅ Excellent |
| ruststack-firehose | 1 | ⚠️ Minimal |
| ruststack-iam | ~5 | ✅ Basic |
| ruststack-lambda | ~10 | ✅ Good |
| ruststack-s3 | ~15 | ✅ Good |
| ruststack-secretsmanager | ~3 | ⚠️ Minimal |
| ruststack-sns | 0 | ❌ None |
| ruststack-sqs | 0 | ❌ None |

**Total: ~102 tests across 12 packages**

## Package-by-Package Test Strategy

### 1. ruststack-cognito ❌ (Priority: HIGH)

Cognito handles user authentication and pool management.

**Current:** No tests

**Required Tests:**
- `UserPool::new()` - pool creation with valid region/name
- `UserPool::create_user()` - user creation flow
- `UserStatus` enum variants - serialization/deserialization
- JWT token generation and validation
- Authentication flow (initiate_auth, respond_to_auth_challenge)
- User attribute handling

**Test File Location:** `ruststack-cognito/src/storage.rs` (add `#[cfg(test)]` module)

### 2. ruststack-sns ❌ (Priority: HIGH)

SNS handles pub/sub messaging and topic management.

**Current:** No tests

**Required Tests:**
- `SnsState::new()` - state initialization
- `Topic::create()` - topic creation
- `Subscription` enum variants - subscription handling
- Message publishing to topics
- Fan-out to SQS
- HTTP/Lambda/Email subscription protocols

**Test File Location:** `ruststack-sns/src/storage.rs`

### 3. ruststack-sqs ❌ (Priority: HIGH)

SQS handles queue management and message processing.

**Current:** No tests

**Required Tests:**
- `Queue::create()` - queue creation
- `Queue::send_message()` - message sending
- `Queue::receive_message()` - message retrieval
- `Queue::delete_message()` - message deletion
- Dead letter queue handling
- FIFO vs standard queue differences
- Message batching

**Test File Location:** `ruststack-sqs/src/storage.rs`

### 4. ruststack-secretsmanager ⚠️ (Priority: MEDIUM)

Secrets Manager handles secure parameter storage.

**Current:** ~3 tests (minimal)

**Required Tests:**
- Secret creation with various data types
- Secret versioning
- Secret rotation simulation
- GetSecretValue flow
- Access policy enforcement

**Test File Location:** `ruststack-secretsmanager/src/storage.rs`

### 5. ruststack-firehose ⚠️ (Priority: MEDIUM)

Firehose handles streaming data to destinations.

**Current:** ~1 test (minimal)

**Required Tests:**
- Delivery stream creation
- Record batching
- Destination types (S3, ES, Redshift)
- Buffering configuration
- Error handling and retries

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

### Phase 1: Critical Services (Week 1-2)
1. **ruststack-sqs** - 15 tests
   - Queue CRUD operations
   - Message lifecycle
   - Batching

2. **ruststack-sns** - 15 tests
   - Topic management
   - Subscription handling
   - Message publishing

3. **ruststack-cognito** - 20 tests
   - User pool management
   - Authentication flows
   - Token handling

### Phase 2: Supporting Services (Week 3-4)
4. **ruststack-secretsmanager** - 10 tests
5. **ruststack-firehose** - 10 tests

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
