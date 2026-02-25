# RustStack CLI

CLI wrapper for [RustStack](https://github.com/eddalmond/ruststack) - High-fidelity AWS local emulator.

## Installation

```bash
# pip
pip install ruststack-cli

# poetry
poetry add --dev ruststack-cli

# uv
uv add --dev ruststack-cli
```

## Quick Start

```bash
# Install ruststack binary
ruststack install

# Start the server
ruststack start

# Check status
ruststack status

# Check for updates
ruststack check-updates
```

## Usage

### CLI Commands

```bash
# Install or update the binary
ruststack install
ruststack install --version 0.1.2  # specific version

# Start the server
ruststack start
ruststack start --port 5000         # custom port
ruststack start --log-level debug   # debug logging
ruststack start --background        # daemon mode

# Check status
ruststack status

# Wait for server to be ready
ruststack wait-ready
ruststack wait-ready --timeout 30

# Check for updates
ruststack check-updates
ruststack update

# Show version
ruststack version
```

### Python Usage

#### Basic Client

```python
from ruststack_cli import RustStackClient

client = RustStackClient(endpoint="http://localhost:4566")

s3 = client.s3()
dynamodb = client.dynamodb()
lambda_ = client.lambda_()
```

#### With Fixtures

```python
# conftest.py
import pytest
from ruststack_cli import ruststack_process, s3_client

@pytest.fixture(scope="session")
def ruststack():
    """Start RustStack for the test session."""
    with ruststack_process() as proc:
        proc.wait_until_ready()
        yield proc.endpoint

@pytest.fixture
def s3(ruststack):
    return s3_client(ruststack)
```

Then in your tests:

```python
def test_s3_bucket_creation(s3):
    s3.create_bucket(Bucket="my-test-bucket")
    response = s3.list_buckets()
    assert "my-test-bucket" in [b["Name"] for b in response["Buckets"]]
```

#### Using the Context Manager

```python
from ruststack_cli import RustStackServer

with RustStackServer.start() as server:
    client = server.client()
    s3 = client.s3()
    # ... run tests
# Server automatically stopped
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUSTSTACK_HOST` | Host to bind to | `127.0.0.1` |
| `RUSTSTACK_PORT` | Port to listen on | `4566` |
| `RUST_LOG` | Log level | `info` |

## Supported Services

- S3 (Buckets, Objects, Multipart Upload)
- DynamoDB (Tables, Items, Query, Scan)
- Lambda (Functions, Invocation)
- SQS (Queues, Messages)
- SNS (Topics, Subscriptions, Publishing)
- Secrets Manager
- API Gateway V2
- CloudWatch Logs
- IAM (Stub)

## Configuration

### Custom Endpoint

```python
client = RustStackClient(endpoint="http://localhost:5000")
```

### Custom AWS Credentials

```python
client = RustStackClient(
    endpoint="http://localhost:4566",
    aws_access_key_id="custom-key",
    aws_secret_access_key="custom-secret",
    region_name="us-west-2",
)
```

## Development

```bash
# Install with dev dependencies
pip install -e ".[dev]"

# Run tests
pytest

# Lint
ruff check src/

# Type check
mypy src/
```

## License

MIT OR Apache-2.0
