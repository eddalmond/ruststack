# Integrating RustStack into Your Integration Tests

This guide shows how to replace `moto`/`localstack` with `ruststack` in your existing integration tests.

## Quick Start (Recommended - Docker)

### Step 1: Add RustStack to docker-compose.yml

Reference the RustStack image from your existing docker-compose:

```yaml
# Your project's docker-compose.yml
services:
  ruststack:
    image: ghcr.io/eddalmond/ruststack:latest
    ports:
      - "4566:4566"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:4566/health"]
      interval: 5s
      timeout: 3s
      retries: 10
```

### Step 2: Update boto3 endpoint URLs

In your `conftest.py`, change all client fixtures from port 5000 (moto) to port 4566 (ruststack):

```python
# OLD (moto on port 5000)
moto_server: URL with port 5000

# NEW (ruststack on port 4566)
ruststack_server: URL with port 4566
```

Minimal code change example:

```python
# In conftest.py, change:
# return boto3_session.client("dynamodb", endpoint_url=str(moto_server))
# To:
return boto3_session.client("dynamodb", endpoint_url="http://localhost:4566")
```

### Step 3: Run tests

```bash
docker-compose up -d ruststack
pytest tests/integration/
docker-compose down
```

That's it! Same tests, different endpoint.

## Running RustStack Integration Tests

### Docker-based Tests

These tests verify RustStack works via HTTP (like LocalStack):

```bash
# Start RustStack
docker run -p 4566:4566 ghcr.io/eddalmond/ruststack:latest

# Or build locally
cargo build --release
./target/release/ruststack --port 4566

# Run tests
pip install boto3 pytest requests
AWS_ENDPOINT_URL=http://localhost:4566 pytest tests/integration/test_docker.py -v
```

### In-Process Tests (Future Enhancement)

Full in-process Python bindings are under development. For now:

```python
# This will work in the future
import ruststack_py

rs = ruststack_py.RustStack()
# Currently returns NotImplementedError
```

The in-process approach requires:
- Async trait support from Rust crates
- Making storage traits publicly accessible
- Thread-safe state management

## Service Coverage

| AWS Service | Docker | In-Process |
|-------------|--------|------------|
| DynamoDB | ✅ Excellent | Future |
| S3 | ✅ Good | Future |
| Secrets Manager | ✅ Good | Future |
| Firehose | ✅ Good | Future |
| IAM | ✅ Basic | Future |
| SNS | ✅ Good | Future |
| SQS | ✅ Excellent | Future |
| Lambda | ⚠️ Limited | N/A |
| API Gateway | ⚠️ Limited | N/A |

For Lambda/API Gateway: keep using LocalStack or real AWS.

## Troubleshooting

### Port Conflicts
If you get port 4566 conflicts, update to use a different port:

```bash
./ruststack --port 4567
# Then use endpoint_url="http://localhost:4567"
```

### Missing Operations
If you encounter unimplemented operations, check the issues or contribute implementations.

### Performance
RustStack is in-memory by default. For large test suites, consider:
- Using separate state per test class (faster cleanup)
- Batch operations where possible
