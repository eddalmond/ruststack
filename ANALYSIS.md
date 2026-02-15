# RustStack Analysis Document

## Executive Summary

This document analyzes LocalStack's implementation of AWS services (S3, DynamoDB, Lambda) to inform the design of RustStack - a high-fidelity AWS local emulator focused on **integration testing for Flask/Lambda applications**.

### Target Use Case

A Flask application running as AWS Lambda, fronted by API Gateway, using:
- **S3** for file storage
- **DynamoDB** for data persistence
- **Lambda** for compute

RustStack prioritizes **depth over breadth** - bulletproof core operations rather than broad but shallow coverage.

---

## 1. Priority Operations

### S3 (High Fidelity Required)

| Operation | Priority | Notes |
|-----------|----------|-------|
| GetObject | **P0** | Range requests, conditional gets |
| PutObject | **P0** | Streaming upload, Content-MD5 |
| DeleteObject | **P0** | Simple delete (no versioning) |
| HeadObject | **P0** | Metadata retrieval |
| ListObjectsV2 | **P0** | Pagination, prefix filtering |
| CreateBucket | P1 | Basic creation |
| DeleteBucket | P1 | Empty bucket only |
| HeadBucket | P1 | Existence check |

**Error codes that must match AWS exactly:**
- `NoSuchKey` (404) - Object doesn't exist
- `NoSuchBucket` (404) - Bucket doesn't exist
- `BucketAlreadyExists` (409) - Bucket name taken
- `BucketNotEmpty` (409) - Delete non-empty bucket
- `InvalidArgument` (400) - Bad request parameters
- `AccessDenied` (403) - Permission denied

### DynamoDB (High Fidelity Required)

| Operation | Priority | Notes |
|-----------|----------|-------|
| GetItem | **P0** | Consistent read support |
| PutItem | **P0** | Condition expressions |
| DeleteItem | **P0** | Condition expressions |
| UpdateItem | **P0** | Update expressions, conditions |
| Query | **P0** | Key conditions, filter expressions, GSI |
| Scan | **P0** | Filter expressions, pagination |
| CreateTable | P1 | GSI support required |
| DeleteTable | P1 | |
| DescribeTable | P1 | Table status |

**Critical behaviors:**
- Condition expression failures → `ConditionalCheckFailedException`
- Item not found on GetItem → empty response (not error)
- GSI queries with correct behavior
- Proper `LastEvaluatedKey` pagination

### Lambda (Medium Fidelity)

| Operation | Priority | Notes |
|-----------|----------|-------|
| Invoke | **P0** | Sync invocation, proper event format |
| CreateFunction | P1 | Zip upload, env vars |
| DeleteFunction | P1 | Cleanup |
| GetFunction | P2 | Inspection |

**Critical behaviors:**
- API Gateway event format (v1) compatibility
- Flask/WSGI handler execution
- Environment variable injection
- Proper error response format

---

## 2. LocalStack S3 Deep Dive

### Core Implementation (`provider.py` - 201KB)

LocalStack's S3 implements operations through handler methods:

```python
@handler("GetObject")
def get_object(self, context: RequestContext, request: GetObjectRequest) -> GetObjectOutput:
    # 1. Validate bucket exists
    # 2. Resolve version (we skip this - no versioning)
    # 3. Check object exists
    # 4. Handle range requests
    # 5. Stream response body
```

**Key behaviors to replicate:**

1. **ETag Calculation:**
   ```python
   etag = hashlib.md5(data).hexdigest()
   # Returns: "d41d8cd98f00b204e9800998ecf8427e"
   # With quotes in HTTP header
   ```

2. **Range Requests:**
   - `Range: bytes=0-99` → first 100 bytes
   - `Range: bytes=-100` → last 100 bytes
   - Returns `206 Partial Content` with `Content-Range` header

3. **Conditional Operations:**
   - `If-Match` / `If-None-Match` with ETag
   - `If-Modified-Since` / `If-Unmodified-Since`
   - Returns `304 Not Modified` or `412 Precondition Failed`

4. **ListObjectsV2 Pagination:**
   ```python
   # MaxKeys default: 1000
   # ContinuationToken: opaque string for next page
   # IsTruncated: true if more results
   ```

### Storage Backend

LocalStack uses `EphemeralS3ObjectStore`:
- `SpooledTemporaryFile` for objects (memory up to 512KB, then disk)
- Thread-safe with reader/writer locks
- MD5 calculated during write

**We can simplify:** Use pure in-memory with `DashMap` since integration tests are ephemeral.

---

## 3. LocalStack DynamoDB Deep Dive

### Critical Insight: LocalStack Proxies to DynamoDB Local

LocalStack does NOT implement DynamoDB's query engine. It:
1. Starts DynamoDB Local (Java) as a subprocess
2. Proxies all requests to it
3. Adds features on top (streams, global tables, ARN fixing)

```python
def forward_request(self, context: RequestContext) -> ServiceResponse:
    self.prepare_request_headers(headers, account_id, region_name)
    return self.server.proxy(context, service_request)
```

### What LocalStack Adds

1. **ARN Transformation:**
   - DynamoDB Local returns `arn:aws:dynamodb:ddblocal:...`
   - LocalStack fixes to `arn:aws:dynamodb:us-east-1:123456789012:...`

2. **Streams (out of scope for us):**
   - Generates stream records on mutations
   - Forwards to DynamoDB Streams API

3. **Error Enhancement:**
   - Better error messages
   - Consistent error codes

### DynamoDB Local Behaviors

DynamoDB Local is **highly compatible** with AWS DynamoDB for:
- All item operations (Get, Put, Update, Delete)
- Query and Scan with expressions
- GSI and LSI
- Condition expressions
- Batch operations

**Our approach:** Use DynamoDB Local as backend, similar to LocalStack.

### Expression Handling Examples

From the tests, critical expression behaviors:

```python
# Condition expression failure
response = client.put_item(
    TableName='test',
    Item={'pk': {'S': 'key1'}},
    ConditionExpression='attribute_not_exists(pk)'
)
# Raises: ConditionalCheckFailedException

# Update expression
response = client.update_item(
    TableName='test',
    Key={'pk': {'S': 'key1'}},
    UpdateExpression='SET #attr = :val',
    ExpressionAttributeNames={'#attr': 'data'},
    ExpressionAttributeValues={':val': {'S': 'new-value'}}
)
```

---

## 4. LocalStack Lambda Deep Dive

### Execution Model

Lambda uses Docker containers with a custom Runtime API:

```
┌─────────────────────────────────────────┐
│  Lambda Container                        │
│  ├── AWS Lambda Runtime (RIC)           │
│  ├── Function Code (/var/task)          │
│  └── Runtime API Client                 │
└─────────────────────────────────────────┘
         │
         ▼ HTTP (localhost:9001)
┌─────────────────────────────────────────┐
│  Runtime API Server (in LocalStack)     │
│  ├── GET  /invocation/next              │
│  ├── POST /invocation/{id}/response     │
│  └── POST /invocation/{id}/error        │
└─────────────────────────────────────────┘
```

### Flask/WSGI Compatibility

For Flask apps, the handler typically uses a wrapper:

```python
# Using aws-wsgi or mangum
from mangum import Mangum
from flask import Flask

app = Flask(__name__)
handler = Mangum(app)
```

The handler receives an **API Gateway v1 event**:

```json
{
  "httpMethod": "GET",
  "path": "/api/users",
  "headers": {"Content-Type": "application/json"},
  "queryStringParameters": {"page": "1"},
  "body": null,
  "isBase64Encoded": false,
  "requestContext": {
    "requestId": "abc123",
    "stage": "prod",
    "httpMethod": "GET",
    "path": "/api/users"
  }
}
```

And returns:

```json
{
  "statusCode": 200,
  "headers": {"Content-Type": "application/json"},
  "body": "{\"users\": []}",
  "isBase64Encoded": false
}
```

### Environment Variables

Critical for Flask apps:
- `AWS_REGION`, `AWS_DEFAULT_REGION`
- `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`
- Custom app config (database URLs, etc.)

Lambda injects these into the container environment.

---

## 5. Existing Rust Projects

### s3s (Recommended for S3)

The [s3s project](https://github.com/s3s-project/s3s) provides:
- Complete S3 API trait (generated from Smithy)
- SigV4 authentication
- File system backend (s3s-fs)
- Active maintenance

**Pros for us:**
- GetObject, PutObject, etc. already structured
- Error types match AWS
- Can implement custom backend

**We should:** Use s3s as foundation, implement `EphemeralStorage` backend.

### DynamoDB Options

No good Rust DynamoDB server exists. Options:
1. **Proxy to DynamoDB Local** (recommended, like LocalStack)
2. Native implementation (massive effort)

---

## 6. Error Fidelity Requirements

### S3 Error Format (XML)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<Error>
    <Code>NoSuchKey</Code>
    <Message>The specified key does not exist.</Message>
    <Key>my-object-key</Key>
    <RequestId>tx00000000000000000001</RequestId>
    <HostId>...</HostId>
</Error>
```

HTTP Status: 404
Headers: `x-amz-request-id`, `x-amz-id-2`

### DynamoDB Error Format (JSON)

```json
{
    "__type": "com.amazonaws.dynamodb.v20120810#ConditionalCheckFailedException",
    "message": "The conditional request failed"
}
```

HTTP Status: 400
Header: `x-amzn-RequestId`

### Lambda Error Format (JSON)

For function errors:
```json
{
    "errorMessage": "division by zero",
    "errorType": "ZeroDivisionError",
    "stackTrace": ["..."]
}
```

Headers: `X-Amz-Function-Error: Unhandled`

---

## 7. Test Strategy

### S3 Compatibility Tests

```rust
#[tokio::test]
async fn test_get_nonexistent_key() {
    let client = create_s3_client().await;
    
    let result = client.get_object()
        .bucket("test-bucket")
        .key("nonexistent")
        .send()
        .await;
    
    let err = result.unwrap_err();
    // Must be NoSuchKey, not generic error
    assert!(err.to_string().contains("NoSuchKey"));
}

#[tokio::test]
async fn test_list_objects_pagination() {
    // Create 1500 objects
    // List with MaxKeys=1000
    // Verify IsTruncated=true
    // Use ContinuationToken
    // Verify all objects retrieved
}
```

### DynamoDB Compatibility Tests

```rust
#[tokio::test]
async fn test_condition_expression_failure() {
    let client = create_dynamodb_client().await;
    
    // Put item
    client.put_item()
        .table_name("test")
        .item("pk", AttributeValue::S("key1".into()))
        .send()
        .await
        .unwrap();
    
    // Put with condition that should fail
    let result = client.put_item()
        .table_name("test")
        .item("pk", AttributeValue::S("key1".into()))
        .condition_expression("attribute_not_exists(pk)")
        .send()
        .await;
    
    assert!(result.is_err());
    // Must be ConditionalCheckFailedException
}
```

### Lambda Integration Test

```rust
#[tokio::test]
async fn test_flask_lambda_invocation() {
    // Create function with Flask app
    // Invoke with API Gateway event
    // Verify response matches Flask route
}
```

---

## 8. Out of Scope (Explicit)

### S3
- ❌ Versioning
- ❌ Lifecycle rules
- ❌ Replication
- ❌ Object lock
- ❌ Notifications
- ❌ Multipart upload (can add later if needed)

### DynamoDB
- ❌ Streams
- ❌ Transactions
- ❌ DAX
- ❌ Global tables
- ❌ Backup/restore
- ❌ Kinesis streaming

### Lambda
- ❌ Layers
- ❌ Provisioned concurrency
- ❌ Destinations
- ❌ Event source mappings
- ❌ Aliases/versions

---

## 9. Recommendations

### Phase 1: S3 Core (Week 1-2)
1. Use s3s framework
2. Implement in-memory storage
3. Focus on: GetObject, PutObject, DeleteObject, HeadObject, ListObjectsV2
4. Perfect error codes

### Phase 2: DynamoDB (Week 3-4)
1. Integrate DynamoDB Local as subprocess
2. Implement proxy with ARN fixing
3. Verify expression handling
4. Test with real Flask app patterns

### Phase 3: Lambda (Week 5-6)
1. Docker container execution
2. API Gateway v1 event format
3. Flask/Mangum compatibility testing
4. Environment variable injection

### Phase 4: Integration (Week 7-8)
1. End-to-end Flask app testing
2. Error scenario coverage
3. Performance baseline
4. Documentation
