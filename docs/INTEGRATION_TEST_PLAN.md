# Integration Test Expansion Plan

## Current State

**Existing Integration Tests:**
- `ruststack-lambda/tests/integration.rs` - Lambda CRUD + invocation tests
- `tests/dynamodb/integration_tests.rs` - DynamoDB table/CRUD operations

**Services with Handlers (testable):**
1. API Gateway (`ruststack-apigateway`)
2. CloudFormation (`ruststack-cloudformation`)
3. Cognito (`ruststack-cognito`)
4. DynamoDB (`ruststack-dynamodb`) ✓ tested
5. Firehose (`ruststack-firehose`)
6. IAM (`ruststack-iam`)
7. Lambda (`ruststack-lambda`) ✓ tested
8. S3 (`ruststack-s3`)
9. SNS (`ruststack-sns`)
10. SQS (`ruststack-sqs`)
11. StepFunctions (`ruststack-stepfunctions`)
12. Secrets Manager (`ruststack-secretsmanager`)

## Phase 1: Individual Service Tests

### Priority 1 - Core Services
| Service | Tests Needed |
|---------|-------------|
| S3 | CreateBucket, PutObject, GetObject, DeleteObject, ListBuckets, ListObjects |
| SQS | CreateQueue, SendMessage, ReceiveMessage, DeleteQueue |
| SNS | CreateTopic, Subscribe, Publish |
| IAM | CreateRole, AttachPolicy, PutUserPolicy |

### Priority 2 - API & Compute
| Service | Tests Needed |
|---------|-------------|
| API Gateway | CreateRestApi, PutMethod, CreateDeployment, Invoke |
| Secrets Manager | CreateSecret, GetSecretValue, PutSecretValue |

### Priority 3 - Advanced Services
| Service | Tests Needed |
|---------|-------------|
| Cognito | CreateUserPool, CreateUserPoolClient |
| Step Functions | CreateStateMachine, StartExecution |
| CloudFormation | CreateStack, DescribeStacks |
| Firehose | CreateDeliveryStream, PutRecord |

## Phase 2: Combined Integration Test

**Scenario: Simple REST API with Backend Storage**

```
┌─────────────────┐     ┌──────────────┐     ┌─────────────┐
│  API Gateway    │────▶│   Lambda     │────▶│  DynamoDB   │
└─────────────────┘     └──────────────┘     └─────────────┘
                               │
                               ▼
                        ┌──────────────┐
                        │      S3      │
                        └──────────────┘
```

**Test Flow:**
1. Create S3 bucket for data storage
2. Create DynamoDB table for metadata
3. Create Lambda function (with S3 + DynamoDB permissions)
4. Create API Gateway REST API
5. Add GET/POST methods linked to Lambda
6. Deploy API
7. Test full flow:
   - POST /items → Lambda writes to S3 + DynamoDB
   - GET /items/{id} → Lambda reads from S3 + DynamoDB
   - GET /items → Lambda lists from DynamoDB
8. Verify data integrity across all services

**Test Coverage:**
- Cross-service authentication (IAM roles)
- API Gateway → Lambda integration
- Lambda → S3/DynamoDB calls
- Error handling across service boundaries

## Implementation Notes

### Test Infrastructure
- Use `TcpListener` to bind to random port
- Create shared state for combined tests
- Each service gets its own router routes
- Use AWS SDK clients for testing

### Test Helpers to Create
```rust
// Common test utilities
async fn start_test_server() -> (port, handles)
async fn create_sdk_client<T>(service, port) -> T
fn create_test_zip() -> Vec<u8>
```

### Naming Convention
- `test_{operation}_{resource}` - individual operations
- `test_{scenario}_{flow}` - integration flows

## Files to Create/Modify

```
tests/
├── s3/
│   ├── mod.rs
│   └── integration_tests.rs
├── sqs/
│   ├── mod.rs
│   └── integration_tests.rs
├── sns/
│   ├── mod.rs
│   └── integration_tests.rs
├── apigateway/
│   ├── mod.rs
│   └── integration_tests.rs
├── iam/
│   ├── mod.rs
│   └── integration_tests.rs
├── secretsmanager/
│   ├── mod.rs
│   └── integration_tests.rs
├── cognito/
│   ├── mod.rs
│   └── integration_tests.rs
├── stepfunctions/
│   ├── mod.rs
│   └── integration_tests.rs
├── cloudformation/
│   ├── mod.rs
│   └── integration_tests.rs
├── firehose/
│   ├── mod.rs
│   └── integration_tests.rs
└── combined/
    ├── mod.rs
    └── multi_service_test.rs  # New combined scenario
```

## Timeline Estimate

| Phase | Services | Effort |
|-------|----------|--------|
| Phase 1 (Priority 1) | S3, SQS, SNS, IAM | 2-3 hours |
| Phase 1 (Priority 2) | API Gateway, Secrets Manager | 2 hours |
| Phase 1 (Priority 3) | Cognito, Step Functions, CloudFormation, Firehose | 2-3 hours |
| Phase 2 | Combined test | 2-3 hours |
| **Total** | | **~10 hours** |
