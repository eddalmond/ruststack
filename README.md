# RustStack

[![CI](https://github.com/eddalmond/ruststack/actions/workflows/ci.yml/badge.svg)](https://github.com/eddalmond/ruststack/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

A high-fidelity AWS local emulator written in Rust. Drop-in replacement for LocalStack for integration testing.

## Why RustStack?

- **Fast** - Starts in milliseconds, not seconds
- **Light** - ~50MB memory vs 300MB+ for LocalStack
- **Simple** - Single binary, no Docker/Java required
- **Compatible** - Same port (4566), same API, same error codes

## Features

| Service | Operations | Notes |
|---------|-----------|-------|
| **S3** | Buckets, objects, multipart upload, copy | XML responses, proper ETags |
| **DynamoDB** | Tables, items, query, scan, batch ops | Full expression support |
| **Lambda** | CRUD, invoke, environment vars | Python subprocess or Docker execution |
| **CloudWatch Logs** | Groups, streams, events | For Lambda log retrieval |
| **Secrets Manager** | Create, get, put, delete, list | Version stages (AWSCURRENT/AWSPREVIOUS) |
| **IAM** | Roles, policies, attachments | Stub implementation (no enforcement) |
| **API Gateway V2** | APIs, routes, integrations, stages | HTTP APIs |
| **Kinesis Firehose** | Delivery streams, put records | In-memory buffering |

## Quick Start

```bash
# Build
cargo build --release

# Run (default port 4566)
./target/release/ruststack

# With debug logging
RUST_LOG=debug ./target/release/ruststack
```

## Usage with boto3

```python
import boto3

endpoint_url = "http://localhost:4566"

# S3
s3 = boto3.client("s3", endpoint_url=endpoint_url,
    aws_access_key_id="test", aws_secret_access_key="test", region_name="us-east-1")

# DynamoDB  
dynamodb = boto3.client("dynamodb", endpoint_url=endpoint_url,
    aws_access_key_id="test", aws_secret_access_key="test", region_name="us-east-1")

# Lambda
lambda_client = boto3.client("lambda", endpoint_url=endpoint_url,
    aws_access_key_id="test", aws_secret_access_key="test", region_name="us-east-1")
```

## pytest Fixture

Replace LocalStack with RustStack in your tests:

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
    
    # Wait for ready
    for _ in range(30):
        try:
            if requests.get("http://localhost:4566/health").status_code == 200:
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
    import boto3
    return boto3.client("s3", endpoint_url=ruststack,
        aws_access_key_id="test", aws_secret_access_key="test", region_name="us-east-1")


@pytest.fixture
def dynamodb_client(ruststack):
    import boto3
    return boto3.client("dynamodb", endpoint_url=ruststack,
        aws_access_key_id="test", aws_secret_access_key="test", region_name="us-east-1")


@pytest.fixture
def lambda_client(ruststack):
    import boto3
    return boto3.client("lambda", endpoint_url=ruststack,
        aws_access_key_id="test", aws_secret_access_key="test", region_name="us-east-1")
```

## Lambda Invocation (API Gateway v2 Format)

```python
import json

event = {
    "version": "2.0",
    "routeKey": "GET /patient-check/{id}",
    "rawPath": "/patient-check/1234567890",
    "rawQueryString": "",
    "headers": {
        "content-type": "application/json",
        "nhs-login-nhs-number": "1234567890",
    },
    "pathParameters": {"id": "1234567890"},
    "requestContext": {
        "http": {"method": "GET", "path": "/patient-check/1234567890"},
        "requestId": "test-request-id",
    },
    "body": None,
    "isBase64Encoded": False,
}

response = lambda_client.invoke(
    FunctionName="my-function",
    InvocationType="RequestResponse",
    Payload=json.dumps(event),
    LogType="Tail",  # Get logs in response
)
```

## CLI Options

```
Usage: ruststack [OPTIONS]

Options:
  -p, --port <PORT>                    Port to listen on [default: 4566]
      --host <HOST>                    Host to bind to [default: 0.0.0.0]
      --lambda-executor <MODE>         Lambda executor: subprocess, docker, auto [default: subprocess]
      --lambda-container-ttl <SECS>    Docker container TTL for warm pool [default: 300]
      --lambda-max-containers <N>      Maximum concurrent Lambda containers [default: 10]
      --lambda-network <MODE>          Docker network mode: bridge or host [default: bridge]
  -h, --help                           Print help
```

## Lambda Execution Modes

RustStack supports two Lambda execution modes:

| Mode | Cold Start | Isolation | Dependencies |
|------|------------|-----------|--------------|
| **subprocess** (default) | ~10-50ms | None | Must be installed on host |
| **docker** | ~500ms-2s | Full container | Bundled in container |

```bash
# Fast development mode (subprocess)
ruststack

# Isolated mode (Docker containers)
ruststack --lambda-executor docker

# Auto mode (Docker for non-Python runtimes)
ruststack --lambda-executor auto
```

See [docs/DOCKER_LAMBDA.md](docs/DOCKER_LAMBDA.md) for detailed Docker configuration.

## Health Check

```bash
curl http://localhost:4566/health
# or LocalStack-compatible:
curl http://localhost:4566/_localstack/health
```

## Supported Operations

### S3
- CreateBucket, DeleteBucket, ListBuckets, HeadBucket
- PutObject, GetObject, DeleteObject, HeadObject, CopyObject
- ListObjects, ListObjectsV2
- CreateMultipartUpload, UploadPart, CompleteMultipartUpload, AbortMultipartUpload

### DynamoDB
- CreateTable, DeleteTable, DescribeTable, ListTables
- PutItem, GetItem, DeleteItem, UpdateItem
- Query, Scan, BatchGetItem, BatchWriteItem
- Full expression support: KeyConditionExpression, FilterExpression, UpdateExpression, ConditionExpression, ProjectionExpression
- GSI and LSI support

### Lambda
- CreateFunction, GetFunction, DeleteFunction, ListFunctions
- Invoke (RequestResponse, Event)
- UpdateFunctionCode, UpdateFunctionConfiguration
- Environment variables, Python runtime

### CloudWatch Logs
- CreateLogGroup, CreateLogStream, DeleteLogGroup
- DescribeLogGroups, DescribeLogStreams
- PutLogEvents, GetLogEvents

### Secrets Manager
- CreateSecret, GetSecretValue, PutSecretValue
- DeleteSecret, DescribeSecret, ListSecrets
- Version stages: AWSCURRENT, AWSPREVIOUS

### IAM (Stub)
- CreateRole, GetRole, DeleteRole, ListRoles
- CreatePolicy, GetPolicy, DeletePolicy
- AttachRolePolicy, DetachRolePolicy, ListAttachedRolePolicies
- Note: IAM is a stub — policies are stored but not enforced

### API Gateway V2 (HTTP APIs)
- CreateApi, GetApi, DeleteApi, GetApis
- CreateRoute, GetRoute, DeleteRoute, GetRoutes
- CreateIntegration, GetIntegration, DeleteIntegration, GetIntegrations
- CreateStage, GetStage, DeleteStage, GetStages

### Kinesis Firehose
- CreateDeliveryStream, DeleteDeliveryStream
- DescribeDeliveryStream, ListDeliveryStreams
- PutRecord, PutRecordBatch
- Note: Records are buffered in memory (not actually delivered to S3)

## Docker

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

```bash
docker build -t ruststack .
docker run -p 4566:4566 ruststack
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Log level/filter | info |
| `RUSTSTACK_LAMBDA_EXECUTOR` | Lambda executor mode | subprocess |
| `RUSTSTACK_LAMBDA_CONTAINER_TTL` | Docker warm container TTL (seconds) | 300 |
| `RUSTSTACK_LAMBDA_MAX_CONTAINERS` | Max concurrent Lambda containers | 10 |
| `RUSTSTACK_LAMBDA_NETWORK` | Docker network mode | bridge |

## Differences from LocalStack

| | RustStack | LocalStack |
|---|-----------|------------|
| Startup | ~10ms | ~5-10s |
| Memory | ~50MB | ~300MB+ |
| Dependencies | None (Docker optional) | Docker, Java |
| Lambda execution | Subprocess or Docker | Container |
| Persistence | In-memory | Optional |
| Services | S3, DynamoDB, Lambda, Logs | 80+ |

## Project Stats

- **~17,500 lines** of Rust
- **240+ tests** with comprehensive coverage
- **CI/CD** via GitHub Actions

## Releases

Tagged releases automatically build binaries for:
- Linux x86_64
- macOS x86_64
- macOS arm64 (Apple Silicon)

```bash
# Create a release
git tag v0.1.0
git push --tags
```

## Contributing

See [ARCHITECTURE.md](ARCHITECTURE.md) for design details and [PLAN.md](PLAN.md) for the roadmap.

## License

MIT OR Apache-2.0
