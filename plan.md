This is a strategic, highly structured development plan optimized specifically for an autonomous LLM coding agent. It translates the business and architectural imperatives from your report into sequential, testable engineering tasks to build RustStack (eddalmond/ruststack), a high-performance, drop-in AWS emulator.

As an AI, I have formatted this roadmap with explicit "Agent Directives," "Acceptance Criteria," and technical boundaries to ensure the executing agent maintains strict adherence to the project's core philosophies: zero telemetry, single-port multiplexing, and deterministic memory usage.

## Table of Contents

- [Phase 0: Foundation & Multiplexing Engine](./plan_phase0.md) ✅ COMPLETED
- [Phase 1: Core Parity & State Engine](./plan_phase1.md) ✅ COMPLETED
- [Phase 2: Compute Emulation & Paywall Breakers](./plan_phase2.md) - Months 4-6
- Phase 2.5: Python Bindings (Priority: HIGH)
- [Phase 3: Advanced Orchestration & Shift-Left Security](./plan_phase3.md) - Months 7-12

---

## Phase 0 Status: COMPLETED

The following tasks have been implemented:

### Task 0.1: Config Management ✅
- Added `EnvConfig` struct with environment variable parsing
- Supported env vars: `SERVICES`, `DEBUG`, `LS_LOG`, `RUSTSTACK_LOG_LEVEL`, `PERSISTENCE`, `LOCALSTACK_HOST`, `USE_SSL`
- Updated `main.rs` to use `EnvConfig::from_env()`

### Task 0.2: Gateway Router ✅
- Added `AwsProtocol` enum for protocol detection
- Added `extract_aws_protocol` middleware to detect AWS protocols (RestJson, Query, Json)
- Added `extract_s3_bucket` middleware for S3 virtual-hosted style routing
- Enhanced logging for debugging

### Task 0.3: SigV4 Interceptor ✅
- Created `ruststack-auth/src/middleware.rs`
- Added `validate_sigv4` middleware that validates SigV4 header structure
- Integrated into router layers

### Files Modified
- `ruststack/src/config.rs` - Added EnvConfig
- `ruststack/src/main.rs` - Updated to use EnvConfig
- `ruststack/src/router.rs` - Added middleware
- `ruststack-auth/src/lib.rs` - Added middleware module
- `ruststack-auth/src/middleware.rs` - New file
- `ruststack-auth/Cargo.toml` - Added dependencies

---

## Phase 1 Status: COMPLETED

### Task 1.1: State Management Engine ✅
- Added rusqlite to workspace dependencies
- Created `ruststack-s3/src/storage/persistent.rs` with SQLite-backed storage
- Added `PersistentStorage` struct implementing `ObjectStorage` trait
- Updated `router.rs` to use persistent storage when `RUSTSTACK_PERSISTENCE=1` or `--persistence` flag is set
- Data stored in SQLite database with filesystem backup for objects

### Task 1.2: S3 Enhancement ✅
- Multipart upload already implemented in handlers and storage
- CORS and versioning tables added to persistent storage (handlers placeholder)
- Virtual-hosted style routing implemented in middleware

### Task 1.3: DynamoDB Enhancement ✅
- UpdateItem, Scan already implemented
- BatchGetItem, BatchWriteItem already implemented
- Query with key conditions already implemented

### Task 1.4: SQS & SNS Enhancement ✅
- SQS core operations (CreateQueue, SendMessage, ReceiveMessage, DeleteMessage) implemented
- SNS core operations (CreateTopic, Publish, Subscribe) implemented
- **SNS → SQS fan-out implemented**: When subscribing an SQS queue to an SNS topic, messages published to the topic are automatically delivered to the SQS queue
- Messages are wrapped in SNS notification format (JSON with Type, MessageId, TopicArn, Subject, Message, Timestamp)

### Files Modified
- `Cargo.toml` - Added rusqlite dependency
- `ruststack-s3/Cargo.toml` - Added rusqlite, parking_lot
- `ruststack-s3/src/storage/traits.rs` - Added Serialize/Deserialize to ObjectMetadata
- `ruststack-s3/src/storage/mod.rs` - Added persistent module
- `ruststack-s3/src/storage/persistent.rs` - New file (SQLite persistence)
- `ruststack-sqs/src/storage.rs` - Added Clone to SqsStorage, added storage() method
- `ruststack-sns/src/storage.rs` - Added SQS fanout callback support
- `ruststack/src/router.rs` - Wired up SNS → SQS fanout
- `ruststack/src/main.rs` - Added persistence flag and data_dir

### Usage
```bash
# Run with persistence enabled
RUSTSTACK_PERSISTENCE=1 cargo run -- --data-dir /tmp/ruststack

# Or use CLI flag
cargo run -- --persistence --data-dir /tmp/ruststack
```

---

## Current Codebase Status

The RustStack project already has significant implementation:

### Implemented Services ✓
- **S3**: Bucket/object operations, path-style routing
- **DynamoDB**: Table CRUD, PutItem, GetItem, Query
- **Lambda**: Function CRUD, subprocess executor, Docker executor
- **Secrets Manager**: Full CRUD operations (in-memory)
- **IAM**: Roles, policies, attach/detach
- **API Gateway V2**: APIs, routes, integrations, stages
- **SQS**: Queue operations
- **SNS**: Topic operations
- **Firehose**: Delivery stream operations
- **CloudWatch Logs**: Log group/stream operations

### Key Files
- Main binary: `ruststack/src/main.rs`
- Router: `ruststack/src/router.rs`
- Config: `ruststack/src/config.rs`

---

🛠️ Global Agent Directives & Constraints

Before initiating any phase, the LLM agent must adhere strictly to these architectural rules:

    Tech Stack: Rust (Edition 2021+), tokio (async runtime), axum (HTTP routing), tracing (logging/telemetry), sqlx or rusqlite (SQLite persistence), aws-smithy-mocks (schema validation).

    Zero-Dependency Guarantee: The final binary must be statically linked (e.g., via cargo-zigbuild) to eliminate the need for heavy Docker base images in CI/CD pipelines.

    Single-Port Architecture: All HTTP traffic must be captured on exactly 0.0.0.0:4566. Do not open separate ports per service.

    No Phoning Home: External network requests are strictly prohibited unless explicitly fetching a user-defined remote resource (e.g., pulling a Lambda Docker image). No telemetry, no authentication checks.

🏗️ Phase 0: Foundation & Multiplexing Engine (Weeks 1-2)

The fundamental requirement of a "drop-in replacement" is capturing and intelligently routing traffic directed at the legacy incumbent's standard port.

Objective: Establish the core HTTP gateway, environment variable configuration, and routing middleware.

    Task 0.1: Project Initialization & Config Management

        Initialize the ruststack workspace.

        Implement a configuration manager to parse and enforce the following exact environment variables:

            SERVICES: Comma-delimited list. If present, only boot listed modules.

            DEBUG / LS_LOG: Map to tracing log levels (trace, debug, info).

            PERSISTENCE: Boolean. If 1, initialize SQLite/File I/O; otherwise, use volatile memory.

            LOCALSTACK_HOST: Default to localhost.localstack.cloud:4566.

            USE_SSL: Boolean toggle for returning https in mock URLs.

    Task 0.2: The Gateway Router (axum)

        Bind an axum server to 0.0.0.0:4566.

        Implement a middleware layer to decode AWS protocols (aws.rest-json-1.1, aws.query, aws.json-1.0).

        Implement intelligent request multiplexing based on the X-Amz-Target header (for POST requests) or the Host header (for S3 virtual-hosted styles).

    Task 0.3: Mock SigV4 Interceptor

        Implement a middleware that validates the structural presence of AWS Signature Version 4 headers to prevent official SDKs from throwing pre-flight errors, immediately returning HTTP 200/Accepted for the handshake.

Phase 0 Acceptance Criteria:
The agent must demonstrate that sending a generic aws s3 ls --endpoint-url http://localhost:4566 command hits the router, logs the raw request via tracing, and returns a mock AWS-formatted error (e.g., NotImplemented) rather than a generic 404.
🗄️ Phase 1: Core Parity & State Engine (Months 1-3)

Implement the most frequently utilized storage and messaging services to facilitate immediate testing against existing Infrastructure as Code (IaC).

Objective: Achieve basic compatibility with tflocal and Terraform overrides for S3, DynamoDB, SQS, and SNS.

    Task 1.1: State Management Engine

        Implement the PERSISTENCE=1 logic.

        Create a local directory manager (e.g., -v ./local-state:/data).

        Initialize a central SQLite database for structured metadata (queue URLs, table schemas).

    Task 1.2: Amazon S3 Implementation

        Implement path-style routing (http://localhost:4566/bucket-name) as enforced by Terraform s3_use_path_style = true.

        Map bucket creation to local directory creation.

        Map object PUT/GET requests directly to standard file system I/O (no RAM bloat).

    Task 1.3: DynamoDB Implementation

        Leverage aws-smithy-mocks to parse DynamoDB JSON payloads.

        Translate basic DynamoDB CreateTable, PutItem, GetItem, and Query operations into SQLite queries.

    Task 1.4: SQS & SNS Implementation

        Create an asynchronous, in-memory message broker using tokio::sync::mpsc channels.

        If persistence is enabled, flush queue states to SQLite.

        Implement basic SNS topic fan-out to local SQS queues.

Phase 1 Acceptance Criteria:
A standard Terraform script utilizing tflocal must successfully provision an S3 bucket, a DynamoDB table, and an SQS queue against the RustStack binary without timing out. Memory utilization must remain under 50MB.
🔓 Phase 2: Compute Emulation & "Paywall Breakers" (Months 4-6)

This phase executes the critical go-to-market strategy: commoditizing the features that the incumbent locked behind commercial tiers ($39-$89/mo).

Objective: Implement serverless compute and highly demanded security/identity services for free.

    Task 2.1: The Paywall Breaker - AWS Secrets Manager

        Implement CRUD operations for Secrets Manager.

        Store secrets securely in the SQLite backend.

        Ensure perfect API parity so local applications can fetch configuration variables on boot without code modifications.

    Task 2.2: The Paywall Breaker - Amazon Cognito

        Implement User Pools and Identity Pools.

        Create a local JWT generation engine. Sign tokens with a mock local private key.

        Provide endpoints for AdminCreateUser, InitiateAuth, and token verification.

    Task 2.3: AWS Lambda & API Gateway

        Implement a local execution engine. Start by allowing developers to mount local directories containing handler code.

        Route API Gateway mock requests to trigger the local Lambda handlers.

        Support Docker-in-Docker execution for containerized Lambdas (ECR emulation).

Phase 2 Acceptance Criteria:
A developer must be able to boot a local web application that successfully fetches a JWT from the mocked Cognito service and retrieves a database password from the mocked Secrets Manager without requiring an active internet connection or a paid license key.

---

## Phase 2.5: Python Bindings (Priority: HIGH)

Enable RustStack to be used directly in Python test suites without Docker, for minimal-overhead local testing.

Objective: Create Python bindings using pyo3 for in-process AWS service mocking.

### Task 2.5.1: Storage Trait Refactoring

Refactor storage crates to expose synchronous (non-async) methods suitable for FFI:

- Review `ruststack-s3`, `ruststack-dynamodb`, `ruststack-secretsmanager`, `ruststack-firehose`, `ruststack-iam`, `ruststack-sns`, `ruststack-sqs`
- Extract async methods into a trait, provide sync implementations alongside
- Make storage modules public for pyo3 access

**Agent Directive:** Focus on making storage implementations accessible without requiring async runtime.

### Task 2.5.2: pyo3 Bindings Core

Create `ruststack-py` crate with pyo3 bindings:

- Set up `ruststack-py/Cargo.toml` with pyo3 dependency
- Implement `RustStack` struct wrapping all service storages
- Bind each service: S3, DynamoDB, Secrets Manager, Firehose, IAM, SNS, SQS
- Handle type conversions between Rust and Python types
- Add proper error handling with Python exceptions

### Task 2.5.3: Build & Distribution

Make bindings easy to install:

- Configure `maturin` for building ✅
- Add `ruststack-py` to CI/CD for wheel building ✅
- Document installation: `pip install ruststack-py`

### Task 2.5.4: Integration Tests

Create comprehensive Python test coverage:

- Add `tests/integration/test_inprocess.py` with full service coverage ✅
- Test all CRUD operations for each service ✅
- Benchmark against Docker method for performance comparison (future work)

Phase 2.5 Acceptance Criteria:
A Python developer must be able to install `ruststack-py`, write `import ruststack_py; rs = ruststack_py.RustStack()`, and use it in pytest fixtures without running any containers.

---

🚀 Phase 3: Advanced Orchestration & Shift-Left Security (Months 7-12)

Solidify RustStack as an enterprise-grade tool by handling complex orchestration and security validations.

Objective: Implement CloudFormation, Step Functions, and deterministic IAM evaluation.

    Task 3.1: CloudFormation Parsing (CDK Support) ✅

        Implement a YAML/JSON parser for AWS CloudFormation templates.

        Build a dependency graph resolver to instantiate mocked resources in the correct order, translating CloudFormation definitions into internal Phase 1 & 2 API calls.

    Task 3.2: AWS Step Functions (Offline ASL) ✅

        Create a parser for the Amazon States Language (ASL).

        Implement a state machine execution engine capable of navigating Choice, Task, and Parallel states, invoking local Lambda functions (from Task 2.3) as defined.

    Task 3.3: Shift-Left Security (Explainable IAM)

        Introduce the ENFORCE_IAM=1 environment variable. ✅

        Implement a deterministic policy evaluation engine in Rust. ✅

        Parse standard AWS JSON policies attached to mocked roles and intercept API calls to validate Allow/Deny rules, returning realistic AccessDeniedException errors when triggered. ✅

Phase 3 Acceptance Criteria:
Executing cdklocal deploy with a template containing a Step Function and strict IAM roles must deploy successfully. Attempting to access an S3 bucket with a local IAM role that lacks s3:GetObject permissions must result in a deterministic access denial.
