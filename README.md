# RustStack

A high-fidelity AWS local emulator written in Rust. Drop-in replacement for LocalStack for integration testing Flask/Lambda applications.

## Features

- **S3** - Complete bucket and object operations with XML error responses
- **DynamoDB** - Full table operations with expression support (KeyConditionExpression, FilterExpression, UpdateExpression)
- **Lambda** - Function management and subprocess-based Python execution
- **CloudWatch Logs** - Log groups, streams, and events for Lambda execution logs
- **Single binary** - No Docker, no Java, just one executable
- **Fast startup** - Ready in milliseconds, not seconds
- **AWS-compatible** - Proper error codes, request IDs, content types

## Quick Start

### Build

```bash
cargo build --release
```

### Run

```bash
# Default port 4566 (LocalStack compatible)
./target/release/ruststack

# Custom port
./target/release/ruststack --port 5000

# With data persistence (coming soon)
./target/release/ruststack --data-dir ./data

# Debug logging
RUST_LOG=debug ./target/release/ruststack
```

### Options

```
Usage: ruststack [OPTIONS]

Options:
  -p, --port <PORT>            Port to listen on [default: 4566]
      --host <HOST>            Host to bind to [default: 0.0.0.0]
      --s3                     Enable S3 service [default: true]
      --dynamodb               Enable DynamoDB service [default: true]
      --lambda                 Enable Lambda service [default: true]
      --data-dir <DATA_DIR>    Data directory for persistence
      --log-level <LOG_LEVEL>  Log level [default: info]
  -h, --help                   Print help
```

## Configuring AWS SDKs

### boto3 (Python)

```python
import boto3

# Create clients pointing at RustStack
endpoint_url = "http://localhost:4566"

s3 = boto3.client(
    "s3",
    endpoint_url=endpoint_url,
    aws_access_key_id="test",
    aws_secret_access_key="test",
    region_name="us-east-1",
)

dynamodb = boto3.client(
    "dynamodb",
    endpoint_url=endpoint_url,
    aws_access_key_id="test",
    aws_secret_access_key="test",
    region_name="us-east-1",
)

lambda_client = boto3.client(
    "lambda",
    endpoint_url=endpoint_url,
    aws_access_key_id="test",
    aws_secret_access_key="test",
    region_name="us-east-1",
)

logs = boto3.client(
    "logs",
    endpoint_url=endpoint_url,
    aws_access_key_id="test",
    aws_secret_access_key="test",
    region_name="us-east-1",
)
```

### AWS CLI

```bash
# Using endpoint URL
aws --endpoint-url http://localhost:4566 s3 ls

# Or set environment variable
export AWS_ENDPOINT_URL=http://localhost:4566
aws s3 ls
```

### Rust (aws-sdk)

```rust
use aws_config::BehaviorVersion;
use aws_sdk_s3::config::Region;

let config = aws_config::defaults(BehaviorVersion::latest())
    .region(Region::new("us-east-1"))
    .endpoint_url("http://localhost:4566")
    .credentials_provider(
        aws_credential_types::Credentials::new(
            "test", "test", None, None, "test"
        )
    )
    .load()
    .await;

let s3_client = aws_sdk_s3::Client::new(&config);
```

## pytest Fixture (LocalStack Replacement)

Replace your LocalStack fixture with RustStack:

```python
import subprocess
import time
import pytest
import requests

@pytest.fixture(scope="session")
def ruststack():
    """Start RustStack for the test session."""
    proc = subprocess.Popen(
        ["./target/release/ruststack", "--port", "4566"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    
    # Wait for server to be ready
    for _ in range(30):
        try:
            resp = requests.get("http://localhost:4566/health")
            if resp.status_code == 200:
                break
        except requests.ConnectionError:
            pass
        time.sleep(0.1)
    else:
        proc.kill()
        raise RuntimeError("RustStack failed to start")
    
    yield "http://localhost:4566"
    
    proc.terminate()
    proc.wait()


@pytest.fixture
def s3_client(ruststack):
    """S3 client pointing at RustStack."""
    import boto3
    return boto3.client(
        "s3",
        endpoint_url=ruststack,
        aws_access_key_id="test",
        aws_secret_access_key="test",
        region_name="us-east-1",
    )


@pytest.fixture
def dynamodb_client(ruststack):
    """DynamoDB client pointing at RustStack."""
    import boto3
    return boto3.client(
        "dynamodb",
        endpoint_url=ruststack,
        aws_access_key_id="test",
        aws_secret_access_key="test",
        region_name="us-east-1",
    )


@pytest.fixture
def lambda_client(ruststack):
    """Lambda client pointing at RustStack."""
    import boto3
    return boto3.client(
        "lambda",
        endpoint_url=ruststack,
        aws_access_key_id="test",
        aws_secret_access_key="test",
        region_name="us-east-1",
    )
```

## API Gateway v2 Event Format

For Lambda invocations, RustStack expects API Gateway v2 (HTTP API) format:

```python
event = {
    "version": "2.0",
    "routeKey": "POST /api/items",
    "rawPath": "/api/items",
    "rawQueryString": "",
    "headers": {
        "content-type": "application/json",
    },
    "requestContext": {
        "http": {
            "method": "POST",
            "path": "/api/items",
        },
        "requestId": "test-request-id",
    },
    "body": json.dumps({"name": "test"}),
    "isBase64Encoded": False,
}

response = lambda_client.invoke(
    FunctionName="my-function",
    Payload=json.dumps(event),
)
```

## Health Check

```bash
# RustStack style
curl http://localhost:4566/health

# LocalStack compatibility
curl http://localhost:4566/_localstack/health
```

Response:
```json
{
  "status": "running",
  "services": {
    "s3": "available",
    "dynamodb": "available",
    "lambda": "available",
    "logs": "available"
  }
}
```

## Docker

Build a Docker image:

```dockerfile
FROM rust:1.75-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/ruststack /usr/local/bin/
EXPOSE 4566
ENTRYPOINT ["ruststack"]
```

Build and run:

```bash
docker build -t ruststack .
docker run -p 4566:4566 ruststack
```

### docker-compose

```yaml
version: '3.8'
services:
  ruststack:
    build: .
    ports:
      - "4566:4566"
    environment:
      - RUST_LOG=info
```

## Supported Operations

### S3

| Operation | Status |
|-----------|--------|
| CreateBucket | ✅ |
| DeleteBucket | ✅ |
| ListBuckets | ✅ |
| PutObject | ✅ |
| GetObject | ✅ |
| DeleteObject | ✅ |
| ListObjects | ✅ |
| ListObjectsV2 | ✅ |
| HeadObject | ✅ |
| CopyObject | ✅ |

### DynamoDB

| Operation | Status |
|-----------|--------|
| CreateTable | ✅ |
| DeleteTable | ✅ |
| DescribeTable | ✅ |
| ListTables | ✅ |
| PutItem | ✅ |
| GetItem | ✅ |
| DeleteItem | ✅ |
| UpdateItem | ✅ |
| Query | ✅ |
| Scan | ✅ |
| BatchGetItem | ✅ |
| BatchWriteItem | ✅ |

Expression support:
- KeyConditionExpression ✅
- FilterExpression ✅
- ProjectionExpression ✅
- UpdateExpression ✅
- ConditionExpression ✅

### Lambda

| Operation | Status |
|-----------|--------|
| CreateFunction | ✅ |
| DeleteFunction | ✅ |
| GetFunction | ✅ |
| ListFunctions | ✅ |
| Invoke | ✅ |
| UpdateFunctionCode | ✅ |
| UpdateFunctionConfiguration | ✅ |

### CloudWatch Logs

| Operation | Status |
|-----------|--------|
| CreateLogGroup | ✅ |
| CreateLogStream | ✅ |
| DescribeLogGroups | ✅ |
| DescribeLogStreams | ✅ |
| GetLogEvents | ✅ |
| PutLogEvents | ✅ |

## Differences from LocalStack

1. **No Docker dependency** - RustStack runs Lambda functions as subprocesses, not containers
2. **In-memory storage** - Data is ephemeral by default (persistence coming soon)
3. **No Pro features** - RustStack focuses on core services for testing
4. **Faster startup** - Milliseconds vs seconds
5. **Lower memory** - ~50MB vs ~300MB+

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUSTSTACK_PORT` | Server port | 4566 |
| `RUSTSTACK_HOST` | Bind address | 0.0.0.0 |
| `RUSTSTACK_S3` | Enable S3 | true |
| `RUSTSTACK_DYNAMODB` | Enable DynamoDB | true |
| `RUSTSTACK_LAMBDA` | Enable Lambda | true |
| `RUSTSTACK_LOG_LEVEL` | Log level | info |
| `RUST_LOG` | Detailed log filter | - |

## Architecture

```
┌─────────────────────────────────────────────────────┐
│              HTTP Gateway (Axum)                    │
│  Port 4566 - Service routing by headers/path       │
└───────────┬───────────┬───────────┬───────────────┘
            │           │           │
    ┌───────▼───┐ ┌─────▼─────┐ ┌───▼─────┐
    │    S3     │ │ DynamoDB  │ │ Lambda  │
    │(In-Memory)│ │(In-Memory)│ │(Subprocess)│
    └───────────┘ └───────────┘ └─────────┘
```

## Contributing

Contributions welcome! Please read the architecture docs in `ARCHITECTURE.md`.

## License

MIT OR Apache-2.0
