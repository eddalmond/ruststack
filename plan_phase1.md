# Phase 1: Core Parity & State Engine

**Objective:** Implement persistent storage, achieve Terraform compatibility, and optimize S3, DynamoDB, SQS, and SNS implementations.

**Timeline:** Months 1-3

---

## Task 1.1: State Management Engine

### Overview
Implement the persistence layer so data survives restarts. Currently uses in-memory (ephemeral) storage. Need to add SQLite/File I/O persistence.

### Current State
- S3: Uses `EphemeralStorage` in `ruststack-s3/src/storage/ephemeral.rs`
- DynamoDB: Uses in-memory `DynamoDBStorage` in `ruststack-dynamodb/src/storage.rs`
- Other services: Various in-memory implementations

### Steps for LLM Agent

1. **Explore current storage implementations:**
   ```bash
   ls -la ruststack-s3/src/storage/
   ls -la ruststack-dynamodb/src/
   ```

2. **Add SQLite dependency to workspace:**
   Edit `Cargo.toml`:
   ```toml
   # Add to workspace.dependencies
   rusqlite = { version = "0.31", features = ["bundled"] }
   ```

3. **Create persistent storage for S3:**
   
   Create `ruststack-s3/src/storage/persistent.rs`:
   ```rust
   use rusqlite::{Connection, params};
   use std::path::Path;
   
   pub struct PersistentStorage {
       conn: Connection,
   }
   
   impl PersistentStorage {
       pub fn new(data_dir: &Path) -> anyhow::Result<Self> {
           std::fs::create_dir_all(data_dir)?;
           let conn = Connection::open(data_dir / "s3.db")?;
           conn.execute(
               "CREATE TABLE IF NOT EXISTS buckets (
                   name TEXT PRIMARY KEY,
                   created_at TEXT NOT NULL
               )",
               [],
           )?;
           conn.execute(
               "CREATE TABLE IF NOT EXISTS objects (
                   bucket TEXT NOT NULL,
                   key TEXT NOT NULL,
                   value BLOB NOT NULL,
                   content_type TEXT,
                   created_at TEXT NOT NULL,
                   PRIMARY KEY (bucket, key)
               )",
               [],
           )?;
           Ok(Self { conn })
       }
       
       // Implement: create_bucket, delete_bucket, put_object, get_object, list_objects, delete_object
   }
   ```

4. **Create storage trait for abstraction:**
   
   Read `ruststack-s3/src/storage/traits.rs` and extend:
   ```rust
   pub trait ObjectStorage: Send + Sync {
       fn create_bucket(&self, name: &str) -> Result<(), S3Error>;
       fn delete_bucket(&self, name: &str) -> Result<(), S3Error>;
       fn list_buckets(&self) -> Result<Vec<Bucket>, S3Error>;
       
       fn put_object(&self, bucket: &str, key: &str, value: Vec<u8>, content_type: Option<&str>) -> Result<(), S3Error>;
       fn get_object(&self, bucket: &str, key: &str) -> Result<Vec<u8>, S3Error>;
       fn delete_object(&self, bucket: &str, key: &str) -> Result<(), S3Error>;
       fn list_objects(&self, bucket: &str, prefix: &str) -> Result<Vec<ObjectInfo>, S3Error>;
   }
   ```

5. **Wire up persistence based on config:**
   
   In `ruststack/src/main.rs`:
   ```rust
   let storage: Arc<dyn ObjectStorage> = if args.persistence {
       Arc::new(PersistentStorage::new(&data_dir)?)
   } else {
       Arc::new(EphemeralStorage::new())
   };
   ```

6. **Apply same pattern to DynamoDB:**
   
   - Create `rusqlite` tables for tables, items, and metadata
   - Implement `PersistentStorage` in `ruststack-dynamodb/src/storage.rs`

7. **Test persistence:**
   ```bash
   # Start with persistence
   RUSTSTACK_DATA_DIR=/tmp/ruststack cargo run &
   
   # Create bucket and object
   aws s3 mb s3://test-bucket --endpoint-url http://localhost:4566
   echo "hello" | aws s3 cp - s3://test-bucket/hello.txt --endpoint-url http://localhost:4566
   
   # Kill and restart
   pkill ruststack
   RUSTSTACK_DATA_DIR=/tmp/ruststack cargo run &
   
   # Verify data persists
   aws s3 ls s3://test-bucket --endpoint-url http://localhost:4566
   ```

---

## Task 1.2: Amazon S3 Implementation

### Overview
S3 is already implemented but needs enhancement for Terraform compatibility and improved path-style routing.

### Current State
- Bucket operations: Create, List, Delete ✓
- Object operations: Put, Get, List, Delete ✓
- Path-style routing ✓
- Virtual-hosted style - needs improvement

### Steps for LLM Agent

1. **Review current S3 implementation:**
   ```bash
   cat ruststack-s3/src/handlers.rs
   cat ruststack-s3/src/service.rs
   ```

2. **Enhance virtual-hosted style routing:**
   
   The router should handle `bucket.localhost:4566` style URLs:
   
   In `ruststack/src/router.rs`, enhance `handle_bucket`:
   ```rust
   async fn handle_bucket(
       State(state): State<Arc<AppState>>,
       Path(bucket): Path<String>,
       method: Method,
       Query(query): Query<ListObjectsQuery>,
       headers: HeaderMap,
       body: Bytes,
   ) -> Response {
       // Check if this is a virtual-hosted request
       // Host header format: bucket.localhost:4566 or bucket.host:4566
       if let Some(host) = headers.get("host") {
           if let Ok(host_str) = host.to_str() {
               // Extract bucket from host if present
               // e.g., "mybucket.localhost:4566" -> bucket = "mybucket"
           }
       }
       // ... existing logic
   }
   ```

3. **Add S3 multipart upload support:**
   
   Required for large file uploads via SDK:
   ```rust
   // In handlers.rs
   pub async fn create_multipart_upload(
       State(state): State<Arc<S3State>>,
       Path((bucket, key)): Path<(String, String)>,
   ) -> Response;
   
   pub async fn upload_part(
       State(state): State<Arc<S3State>>,
       Path((bucket, key, upload_id)): Path<(String, String, String)>,
       body: Bytes,
   ) -> Response;
   
   pub async fn complete_multipart_upload(
       State(state): State<Arc<S3State>>,
       Path((bucket, key, upload_id)): Path<(String, String, String)>,
       body: Bytes,
   ) -> Response;
   ```

4. **Add S3 CORS support:**
   ```rust
   pub async fn put_bucket_cors(...) -> Response;
   pub async fn get_bucket_cors(...) -> Response;
   ```

5. **Add S3 versioning (basic):**
   ```rust
   // Enable versioning per bucket
   pub async fn put_bucket_versioning(...) -> Response;
   pub async fn get_bucket_versioning(...) -> Response;
   ```

6. **Test with Terraform:**
   ```hcl
   provider "aws" {
     endpoint = "http://localhost:4566"
     region = "us-east-1"
     access_key = "test"
     secret_key = "test"
     s3_use_path_style = true
   }
   
   resource "aws_s3_bucket" "test" {
     bucket = "terraform-test-bucket"
   }
   ```
   
   Run: `terraform init && terraform apply`

7. **Acceptance Test:**
   - Terraform S3 bucket creation works
   - Path-style: `http://localhost:4566/bucket/key`
   - Virtual-hosted: `http://bucket.localhost:4566/key`
   - Large file uploads work via multipart

---

## Task 1.3: DynamoDB Implementation

### Overview
DynamoDB is partially implemented. Need to add more operations and improve query expression support.

### Current State
- CreateTable, DeleteTable ✓
- PutItem, GetItem, DeleteItem ✓
- Basic Query support - needs enhancement
- Scan - needs implementation
- UpdateItem - needs implementation

### Steps for LLM Agent

1. **Review current DynamoDB implementation:**
   ```bash
   cat ruststack-dynamodb/src/handlers.rs
   cat ruststack-dynamodb/src/expression.rs
   ```

2. **Add missing DynamoDB operations:**

   a. **UpdateItem:**
   ```rust
   pub async fn update_item(
       State(state): State<Arc<DynamoDBState>>,
       headers: HeaderMap,
       body: Bytes,
   ) -> Response {
       // Parse UpdateItem request
       // Handle: SET, REMOVE, ADD, DELETE clauses
       // Return updated item
   }
   ```

   b. **Scan:**
   ```rust
   pub async fn scan(
       State(state): State<Arc<DynamoDBState>>,
       headers: HeaderMap,
       body: Bytes,
   ) -> Response {
       // Scan all items in table
       // Support: FilterExpression, ProjectionExpression, Limit
   }
   ```

   c. **Batch operations:**
   ```rust
   pub async fn batch_write_item(...) -> Response;
   pub async fn batch_get_item(...) -> Response;
   ```

3. **Enhance Query support:**
   
   Improve `ruststack-dynamodb/src/expression.rs`:
   - Add support for more comparison operators: `BETWEEN`, `begins_with`, `attribute_exists`, `attribute_not_exists`
   - Add support for `KeyConditionExpression`
   - Add support for `FilterExpression`

4. **Add DynamoDB Local secondary indexes (optional):**
   ```rust
   // If defined in CreateTable request
   pub struct LocalSecondaryIndex {
       index_name: String,
       key_schema: Vec<KeySchemaElement>,
       projection: Projection,
   }
   ```

5. **Test with Terraform:**
   ```hcl
   resource "aws_dynamodb_table" "test" {
     name           = "terraform-test-table"
     billing_mode   = "PAY_PER_REQUEST"
     hash_key       = "id"
     attribute {
       name = "id"
       type = "S"
     }
   }
   ```

6. **Acceptance Test:**
   - Terraform DynamoDB table creation works
   - `aws dynamodb put-item` works
   - `aws dynamodb get-item` works
   - `aws dynamodb query` works with key conditions
   - `aws dynamodb scan` works

---

## Task 1.4: SQS & SNS Implementation

### Overview
SQS and SNS are partially implemented. Need to add persistence and proper fan-out support.

### Current State
- SQS: Basic queue operations implemented
- SNS: Basic topic operations implemented
- SNS → SQS fan-out: Not implemented
- Persistence: Not implemented

### Steps for LLM Agent

1. **Review current implementations:**
   ```bash
   cat ruststack-sqs/src/handlers.rs
   cat ruststack-sns/src/handlers.rs
   cat ruststack-sqs/src/storage.rs
   cat ruststack-sns/src/storage.rs
   ```

2. **Add SQS persistence:**
   
   In `ruststack-sqs/src/storage.rs`:
   ```rust
   use rusqlite::Connection;
   use std::path::Path;
   
   pub struct PersistentQueueStorage {
       conn: Connection,
   }
   
   impl PersistentQueueStorage {
       pub fn new(data_dir: &Path) -> anyhow::Result<Self> {
           std::fs::create_dir_all(data_dir)?;
           let conn = Connection::open(data_dir / "sqs.db")?;
           conn.execute(
               "CREATE TABLE IF NOT EXISTS queues (
                   url TEXT PRIMARY KEY,
                   name TEXT NOT NULL,
                   created_at TEXT NOT NULL,
                   attributes TEXT
               )",
               [],
           )?;
           conn.execute(
               "CREATE TABLE IF NOT EXISTS messages (
                   queue_url TEXT NOT NULL,
                   receipt_handle TEXT PRIMARY KEY,
                   body TEXT NOT NULL,
                   visible_after TEXT NOT NULL,
                   created_at TEXT NOT NULL
               )",
               [],
           )?;
           Ok(Self { conn })
       }
   }
   ```

3. **Add SNS persistence:**
   
   Similar to SQS in `ruststack-sns/src/storage.rs`

4. **Implement SNS → SQS fan-out:**
   
   When publishing to SNS topic, check for subscribed SQS queues and deliver:
   
   In `ruststack-sns/src/handlers.rs`:
   ```rust
   pub async fn publish(
       State(state): State<Arc<SnsState>>,
       headers: HeaderMap,
       body: Bytes,
   ) -> Response {
       let request: PublishInput = serde_json::from_slice(&body).unwrap();
       
       // Get topic
       let topic = state.get_topic(&request.topic_arn);
       
       // Publish to topic
       let message_id = topic.publish(&request.message).await;
       
       // Fan-out to SQS subscriptions
       for subscription in &topic.subscriptions {
           if subscription.protocol == "sqs" {
               // Send to SQS queue
               send_to_sqs(&subscription.endpoint, &request.message).await;
           }
       }
       
       // Return response
   }
   ```

5. **Add SNS subscription confirmation:**
   
   SNS requires subscription confirmation via URL:
   ```rust
   pub async fn subscribe(
       State(state): State<Arc<SnsState>>,
       headers: HeaderMap,
       body: Bytes,
   ) -> Response {
       // Handle SubscriptionConfirmation for HTTP endpoints
       // Return proper response based on protocol
   }
   ```

6. **Test SNS → SQS:**
   ```bash
   # Create SQS queue
   aws sqs create-queue --queue-name test-queue --endpoint-url http://localhost:4566
   
   # Create SNS topic
   aws sns create-topic --name test-topic --endpoint-url http://localhost:4566
   
   # Subscribe SQS to SNS
   aws sns subscribe --topic-arn arn:aws:sns:us-east-1:000000000000:test-topic \
     --protocol sqs \
     --notification-endpoint http://localhost:4566/000000000000/test-queue \
     --endpoint-url http://localhost:4566
   
   # Publish to topic
   aws sns publish --topic-arn arn:aws:sns:us-east-1:000000000000:test-topic \
     --message "test message" \
     --endpoint-url http://localhost:4566
   
   # Receive from SQS
   aws sqs receive-message --queue-url http://localhost:4566/000000000000/test-queue
   ```

7. **Acceptance Test:**
   - SQS queues persist across restarts
   - SNS topics persist across restarts
   - SNS → SQS fan-out works
   - Terraform can create SQS queues and SNS topics

---

## Phase 1 Acceptance Criteria Summary

| Criterion | Test Command | Expected Result |
|-----------|--------------|-----------------|
| S3 persistence | Create bucket, restart, list buckets | Bucket still exists |
| S3 Terraform | `terraform apply` with S3 resource | Bucket created |
| S3 path-style | `curl localhost:4566/bucket/key` | Works |
| DynamoDB UpdateItem | AWS CLI update-item | Item updated |
| DynamoDB Scan | AWS CLI scan | All items returned |
| DynamoDB Terraform | `terraform apply` with DynamoDB | Table created |
| SQS persistence | Create queue, restart, list queues | Queue exists |
| SNS → SQS fan-out | Publish to SNS | Message delivered to SQS |
| Memory usage | Run with data | Under 50MB for basic operations |

---

## Notes for LLM Agent

- **Key files:**
  - S3 handlers: `ruststack-s3/src/handlers.rs`
  - S3 storage: `ruststack-s3/src/storage/`
  - DynamoDB handlers: `ruststack-dynamodb/src/handlers.rs`
  - SQS: `ruststack-sqs/src/`
  - SNS: `ruststack-sns/src/`

- **Database location:**
  - Default: `./data/s3.db`, `./data/dynamodb.db`, etc.
  - Configurable via `RUSTSTACK_DATA_DIR`

- **Testing:**
  - Use AWS CLI for manual testing
  - Use Terraform for integration testing
  - Add unit tests in respective `src/` directories
