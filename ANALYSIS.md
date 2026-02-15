# RustStack Analysis Document

## Executive Summary

This document analyzes LocalStack's implementation of AWS services (S3, DynamoDB, Lambda) to inform the design of RustStack - a high-fidelity Rust-based AWS local emulator.

## 1. LocalStack Architecture Overview

### Core Design Patterns

LocalStack uses a **provider-based architecture** where each AWS service is implemented as a Python class that:
1. Inherits from an auto-generated API interface (from AWS Smithy models)
2. Uses handler decorators to map operations
3. Maintains state through `BaseStore` classes
4. Supports cross-account and cross-region attributes

**Key Files:**
- `localstack/aws/api/` - Auto-generated API types from Smithy
- `localstack/services/*/provider.py` - Service implementations
- `localstack/services/*/models.py` - Data models and state stores
- `localstack/services/stores.py` - Base store infrastructure

### State Management

LocalStack uses `AccountRegionBundle` for multi-tenant state:
```python
dynamodb_stores = AccountRegionBundle("dynamodb", DynamoDBStore)
```

Key store types:
- `LocalAttribute` - Region-scoped data
- `CrossRegionAttribute` - Account-wide data
- `CrossAccountAttribute` - Global data

---

## 2. S3 Implementation Analysis

### File Structure
```
localstack/services/s3/
├── provider.py      (201KB) - Main S3 API implementation
├── models.py        (31KB)  - S3Bucket, S3Object, KeyStore, etc.
├── storage/
│   ├── core.py      (7KB)   - Abstract storage interface
│   └── ephemeral.py (18KB)  - In-memory storage implementation
├── notifications.py (32KB)  - Event notification handling
├── presigned_url.py (38KB)  - Presigned URL generation/validation
├── cors.py          (13KB)  - CORS handling
├── utils.py         (44KB)  - Utilities (checksums, ETags, etc.)
├── validation.py    (20KB)  - Input validation
└── website_hosting.py (16KB) - Static website hosting
```

### API Operations Implemented (~95 operations)

**Bucket Operations:**
- CreateBucket, DeleteBucket, ListBuckets, HeadBucket
- GetBucketLocation, GetBucketVersioning, PutBucketVersioning
- GetBucketEncryption, PutBucketEncryption, DeleteBucketEncryption
- GetBucketNotificationConfiguration, PutBucketNotificationConfiguration
- GetBucketTagging, PutBucketTagging, DeleteBucketTagging
- GetBucketCors, PutBucketCors, DeleteBucketCors
- GetBucketLifecycleConfiguration, PutBucketLifecycleConfiguration
- GetBucketPolicy, PutBucketPolicy, DeleteBucketPolicy
- GetBucketAcl, PutBucketAcl
- GetBucketWebsite, PutBucketWebsite, DeleteBucketWebsite
- GetBucketLogging, PutBucketLogging
- GetBucketReplication, PutBucketReplication, DeleteBucketReplication
- GetPublicAccessBlock, PutPublicAccessBlock, DeletePublicAccessBlock
- GetBucketOwnershipControls, PutBucketOwnershipControls
- GetBucketAccelerateConfiguration, PutBucketAccelerateConfiguration
- GetBucketRequestPayment, PutBucketRequestPayment
- Analytics, Inventory, Metrics, IntelligentTiering configurations

**Object Operations:**
- PutObject, GetObject, HeadObject, DeleteObject, DeleteObjects
- CopyObject
- GetObjectAttributes
- RestoreObject (partial)
- GetObjectAcl, PutObjectAcl
- GetObjectTagging, PutObjectTagging, DeleteObjectTagging
- GetObjectLockConfiguration, PutObjectLockConfiguration
- GetObjectLegalHold, PutObjectLegalHold
- GetObjectRetention, PutObjectRetention

**Multipart Upload:**
- CreateMultipartUpload, UploadPart, UploadPartCopy
- CompleteMultipartUpload, AbortMultipartUpload
- ListParts, ListMultipartUploads

**Versioning:**
- Full version support in GetObject, DeleteObject, HeadObject
- ListObjectVersions

### Storage Backend Architecture

LocalStack uses an abstract `S3ObjectStore` with implementations:

```python
class S3ObjectStore(abc.ABC):
    def open(bucket, s3_object, mode) -> S3StoredObject
    def remove(bucket, s3_object)
    def copy(src_bucket, src_object, dest_bucket, dest_object)
    def get_multipart(bucket, upload_id) -> S3StoredMultipart
    def remove_multipart(bucket, s3_multipart)
```

**EphemeralS3ObjectStore:**
- Uses `SpooledTemporaryFile` for in-memory/disk hybrid storage
- 512KB threshold for in-memory vs disk
- Thread-safe with read/write locks
- Calculates MD5/checksums during write

### Key Behaviors & Edge Cases

1. **Versioning:**
   - `version_id=None` for unversioned buckets
   - `version_id="null"` when versioning suspended
   - Delete markers are special version entries
   - Versioned KeyStore maintains ordered version lists

2. **ETags:**
   - MD5 hash for single uploads: `"md5hex"`
   - Multipart: `"md5hex-partcount"`
   - Content-MD5 header validation

3. **Checksums:**
   - CRC32, CRC32C, SHA1, SHA256, CRC64NVME
   - Calculated during upload, validated on retrieval

4. **Presigned URLs:**
   - SigV2 and SigV4 support
   - Query string authentication
   - Expiration validation

5. **Notifications:**
   - EventBridge, SNS, SQS, Lambda destinations
   - Event filtering by prefix/suffix
   - Event types: s3:ObjectCreated:*, s3:ObjectRemoved:*, etc.

---

## 3. DynamoDB Implementation Analysis

### File Structure
```
localstack/services/dynamodb/
├── provider.py  (100KB) - DynamoDB API implementation
├── models.py    (4KB)   - Minimal models (delegates to DynamoDB Local)
├── server.py    (8KB)   - DynamoDB Local server management
├── utils.py     (15KB)  - Helper functions
└── v2/          - CloudFormation resource handlers
```

### Architecture Approach

**Critical Design Decision:** LocalStack uses **DynamoDB Local** as the backend!

```python
class DynamoDBProvider(DynamodbApi, ServiceLifecycleHook):
    server: DynamodbServer  # Instance of DynamoDB Local
    
    def forward_request(self, context, service_request):
        return self.server.proxy(context, service_request)
```

This means LocalStack:
- Doesn't implement DynamoDB storage/query engine
- Acts as a proxy with pre/post processing
- Adds features DynamoDB Local lacks

### Features Added on Top of DynamoDB Local

1. **Streams & Event Forwarding:**
   - DynamoDB Streams (to `dynamodbstreams` service)
   - Kinesis streaming destinations
   - Stream record generation with OLD_IMAGE/NEW_IMAGE

2. **Global Tables (v2017 & v2019):**
   - Cross-region table replication simulation
   - `TABLE_REGION` and `REPLICAS` tracking

3. **TTL (Time-to-Live):**
   - Background worker deletes expired items
   - `ExpiredItemsWorker` runs hourly

4. **ARN Fixing:**
   - Converts DynamoDB Local ARNs to proper AWS format

5. **Server-Side Encryption (SSE):**
   - KMS key management simulation

6. **Tagging:**
   - Table ARN → tags mapping

### API Operations

**Fully Proxied (with modifications):**
- CreateTable, DeleteTable, DescribeTable, UpdateTable
- PutItem, GetItem, UpdateItem, DeleteItem
- Query, Scan
- BatchWriteItem, BatchGetItem
- TransactWriteItems, TransactGetItems
- ExecuteStatement, BatchExecuteStatement
- DescribeTimeToLive, UpdateTimeToLive
- ListTables, ListTagsOfResource

**Custom Implementation:**
- CreateGlobalTable, UpdateGlobalTable (v2017)
- DescribeGlobalTable, ListGlobalTables
- EnableKinesisStreamingDestination
- DescribeContinuousBackups
- TagResource, UntagResource

### Edge Cases & Behaviors

1. **Throughput Throttling:**
   - Simulates ProvisionedThroughputExceededException
   - Configurable via environment variables

2. **Consumed Capacity:**
   - Fixes response to include proper capacity units

3. **Region Handling:**
   - "localhost" region mapped to us-east-1
   - Global table region routing

---

## 4. Lambda Implementation Analysis

### File Structure
```
localstack/services/lambda_/
├── provider.py         (203KB) - Main Lambda API
├── api_utils.py        (31KB)  - ARN parsing, validation
├── invocation/
│   ├── lambda_service.py    (34KB) - Invocation orchestration
│   ├── lambda_models.py     (21KB) - Function, Version, Alias models
│   ├── version_manager.py   (15KB) - Version lifecycle
│   ├── event_manager.py     (27KB) - Async invocation handling
│   ├── docker_runtime_executor.py (21KB) - Container execution
│   ├── execution_environment.py   (18KB) - Runtime environment
│   └── executor_endpoint.py (11KB) - Runtime API server
├── event_source_mapping/
│   └── esm_worker.py        - Event source mapping workers
├── runtimes.py              - Runtime image mappings
└── layerfetcher/            - Layer handling
```

### Execution Model

Lambda uses **containerized execution** with a custom Runtime Interface Client (RIC):

```
┌─────────────────────────────────────────────────────────┐
│  Lambda Provider                                         │
│  ├── LambdaService                                      │
│  │   ├── LambdaVersionManager (per qualified ARN)      │
│  │   ├── AssignmentService (container pool)            │
│  │   └── CountingService (concurrency tracking)        │
│  └── LambdaEventManager (async/queue handling)         │
└─────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│  Docker Container                                        │
│  ├── /var/rapid/init (LocalStack RIC)                  │
│  ├── /var/task/ (function code)                        │
│  └── /opt/ (layers)                                    │
│                                                         │
│  Runtime API: http://localhost:9001                     │
│  ├── /runtime/invocation/next                          │
│  ├── /runtime/invocation/{id}/response                 │
│  └── /runtime/invocation/{id}/error                    │
└─────────────────────────────────────────────────────────┘
```

### API Operations Implemented

**Function Management:**
- CreateFunction, UpdateFunctionCode, UpdateFunctionConfiguration
- GetFunction, GetFunctionConfiguration, ListFunctions
- DeleteFunction
- PublishVersion, ListVersionsByFunction

**Aliases:**
- CreateAlias, GetAlias, UpdateAlias, DeleteAlias, ListAliases
- Weighted alias routing

**Invocation:**
- Invoke (sync/async/dry-run)
- InvokeAsync (deprecated API)

**Event Source Mappings:**
- CreateEventSourceMapping, UpdateEventSourceMapping
- DeleteEventSourceMapping, GetEventSourceMapping
- ListEventSourceMappings
- Sources: SQS, Kinesis, DynamoDB Streams, Kafka

**Function URLs:**
- CreateFunctionUrlConfig, GetFunctionUrlConfig
- UpdateFunctionUrlConfig, DeleteFunctionUrlConfig
- ListFunctionUrlConfigs

**Permissions:**
- AddPermission, RemovePermission, GetPolicy

**Layers:**
- PublishLayerVersion, GetLayerVersion, DeleteLayerVersion
- ListLayers, ListLayerVersions
- AddLayerVersionPermission, RemoveLayerVersionPermission

**Concurrency:**
- PutFunctionConcurrency, GetFunctionConcurrency
- DeleteFunctionConcurrency
- PutProvisionedConcurrencyConfig, GetProvisionedConcurrencyConfig

**Configuration:**
- PutFunctionEventInvokeConfig
- GetFunctionEventInvokeConfig
- UpdateFunctionEventInvokeConfig

### Execution Flow

1. **Function Creation:**
   - Validate runtime, architecture, VPC config
   - Store code (S3 reference or zip)
   - Initialize VersionManager

2. **Invocation:**
   - Route through alias/version resolution
   - Check concurrency limits
   - Acquire container from pool
   - Send payload via Runtime API
   - Return response or queue for async

3. **Container Management:**
   - Pre-built images per runtime
   - Hot reloading support
   - Container reuse with keep-alive

### Key Behaviors

1. **State Machine:**
   - Pending → Active/Failed
   - LastUpdateStatus tracking
   - SnapStart optimization

2. **Error Handling:**
   - ResourceNotFoundException, InvalidParameterValueException
   - Timeout handling with max 900s
   - Dead letter queues for async failures

3. **Hot Reloading:**
   - `LOCALSTACK_HOT_RELOADING_PATHS` env var
   - File watch and container restart

---

## 5. Existing Rust Projects Analysis

### s3s (s3s-project/s3s)

**Pros:**
- Production-quality S3 implementation framework
- Generated from AWS Smithy models
- Full S3 trait with ~100 operations
- SigV4/SigV2 authentication
- File system backend (s3s-fs)
- Hyper/Tower based HTTP handling
- Active development (2024-2025)

**Cons:**
- S3-only (no DynamoDB/Lambda)
- Designed as framework, not standalone server
- Missing some advanced features (notifications, replication)

**Technical Details:**
- Rust 1.88+, edition 2024
- Uses axum/hyper for HTTP
- CRC32, CRC32C, SHA1, SHA256 checksums
- OpenDAL integration for storage

**Code Quality:**
- Clippy pedantic compliance
- Comprehensive error handling
- Generated code from Smithy

### rusoto_mock

**Status:** Deprecated (rusoto is archived)
**Use:** Historical reference only

### Other Notable Projects

- **minio-rs**: Client library, not server
- **garage**: Distributed S3, too complex for local dev
- **OpenDAL**: Object storage abstraction layer

---

## 6. Critical Behaviors for AWS Fidelity

### Error Response Format

AWS returns XML errors with specific structure:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<Error>
    <Code>NoSuchBucket</Code>
    <Message>The specified bucket does not exist</Message>
    <BucketName>example-bucket</BucketName>
    <RequestId>tx00000000000000000001-00...</RequestId>
    <HostId>...</HostId>
</Error>
```

### Request Authentication

1. **Signature Version 4:**
   - Authorization header parsing
   - Canonical request generation
   - String-to-sign calculation
   - Streaming signatures (chunked)

2. **Presigned URLs:**
   - Query parameter authentication
   - X-Amz-* headers in query string
   - Expiration enforcement

### Consistency Model

AWS S3 is strongly consistent as of 2020. Key behaviors:
- PUT then GET returns latest
- DELETE then GET returns 404
- List operations see latest objects

### Request ID Generation

- `x-amz-request-id`: 16 hex chars (S3), UUID (DynamoDB)
- `x-amz-id-2`: Base64 encoded extended ID

---

## 7. Recommendations for RustStack

### Phase 1: S3 Priority

1. **Use s3s as foundation** - Don't reinvent HTTP/auth layer
2. **Implement robust storage backend** - Start with ephemeral, plan for persistent
3. **Focus on core operations first:**
   - Object CRUD (Get, Put, Delete, Head, Copy)
   - Multipart upload
   - Versioning
   - Bucket operations

### Phase 2: DynamoDB

Two options:

**Option A: Embed DynamoDB Local (Java)**
- Pros: Full compatibility, proven
- Cons: JVM dependency, large footprint

**Option B: Native Implementation**
- Pros: Pure Rust, smaller footprint
- Cons: Massive undertaking (query engine, indexes, expressions)

**Recommendation:** Start with Option A (like LocalStack), add native as stretch goal.

### Phase 3: Lambda

- Container-based execution is correct approach
- Runtime API is well-documented
- Focus on basic invocation first, add event sources later

### Test Strategy

1. **AWS SDK Compatibility Tests:**
   - Use official aws-sdk-rust
   - Run against RustStack and verify behavior

2. **Snapshot Testing:**
   - Capture response structures
   - Compare with LocalStack snapshots

3. **Integration Tests:**
   - Real AWS comparison where safe
   - Mock time/randomness for determinism

---

## Appendix A: LocalStack Test Coverage Analysis

From test files in `tests/aws/services/`:

### S3 Tests (~30 test files)
- `test_s3_api.py` - CRUD, versioning, delete markers
- `test_s3_list_operations.py` - ListObjects, ListObjectsV2, pagination
- `test_s3_preconditions.py` - If-Match, If-None-Match headers
- `test_s3_cors.py` - CORS configuration and preflight
- `test_s3_notifications_*.py` - Event notifications to SQS/SNS/Lambda/EventBridge
- `test_s3_concurrency.py` - Concurrent upload handling

### DynamoDB Tests
- `test_dynamodb.py` - Core operations
- Item expressions, condition expressions
- Global tables
- Streams integration

### Lambda Tests
- Function lifecycle
- Invocation modes
- Event source mappings
- Layers
- Concurrency

---

## Appendix B: API Surface Comparison

| Service | AWS Operations | LocalStack Implemented | RustStack Target |
|---------|---------------|----------------------|------------------|
| S3 | ~100 | ~95 | 50 (core) |
| DynamoDB | ~50 | ~45 | 30 (core) |
| Lambda | ~60 | ~55 | 30 (core) |

Priority operations for MVP are marked in PLAN.md.
