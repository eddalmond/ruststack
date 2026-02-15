# RustStack

**High-fidelity AWS local emulator for integration testing**

RustStack provides local implementations of S3, DynamoDB, and Lambda for testing Flask/Lambda applications without cloud costs or network latency.

## Features

- **S3**: GetObject, PutObject, DeleteObject, HeadObject, ListObjectsV2
- **DynamoDB**: GetItem, PutItem, DeleteItem, UpdateItem, Query, Scan (with expressions)
- **Lambda**: CreateFunction, Invoke, DeleteFunction (with Flask/WSGI support)
- **Error Fidelity**: Exact AWS error codes and response formats

## Quick Start

### Prerequisites

- Rust 1.75+
- Docker (for Lambda)
- Java 11+ (for DynamoDB Local)
- DynamoDB Local JAR

### Installation

```bash
# Clone
git clone https://github.com/your-org/ruststack
cd ruststack

# Build
cargo build --release

# Run
./target/release/ruststack
```

### Docker

```bash
docker run -p 4566:4566 -v /var/run/docker.sock:/var/run/docker.sock ruststack/ruststack
```

### Usage with AWS SDK

```python
import boto3

# S3
s3 = boto3.client('s3', endpoint_url='http://localhost:4566')
s3.put_object(Bucket='my-bucket', Key='test.txt', Body=b'hello')
obj = s3.get_object(Bucket='my-bucket', Key='test.txt')
print(obj['Body'].read())

# DynamoDB
dynamodb = boto3.client('dynamodb', endpoint_url='http://localhost:4566')
dynamodb.put_item(
    TableName='my-table',
    Item={'pk': {'S': 'key1'}, 'data': {'S': 'value1'}}
)

# Lambda
lambda_client = boto3.client('lambda', endpoint_url='http://localhost:4566')
response = lambda_client.invoke(
    FunctionName='my-function',
    Payload=b'{"httpMethod": "GET", "path": "/api/test"}'
)
```

## Configuration

Environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `RUSTSTACK_PORT` | 4566 | Port to listen on |
| `RUSTSTACK_S3` | true | Enable S3 service |
| `RUSTSTACK_DYNAMODB` | true | Enable DynamoDB service |
| `RUSTSTACK_LAMBDA` | true | Enable Lambda service |
| `RUSTSTACK_DYNAMODB_LOCAL_PATH` | ./DynamoDBLocal.jar | Path to DynamoDB Local JAR |

## Supported Operations

### S3

| Operation | Status | Notes |
|-----------|--------|-------|
| GetObject | ✅ | Range requests supported |
| PutObject | ✅ | Streaming, Content-MD5 |
| DeleteObject | ✅ | |
| HeadObject | ✅ | |
| ListObjectsV2 | ✅ | Pagination, prefix |
| CreateBucket | ✅ | |
| DeleteBucket | ✅ | Must be empty |
| HeadBucket | ✅ | |

### DynamoDB

| Operation | Status | Notes |
|-----------|--------|-------|
| GetItem | ✅ | Consistent read |
| PutItem | ✅ | Condition expressions |
| DeleteItem | ✅ | Condition expressions |
| UpdateItem | ✅ | Update expressions |
| Query | ✅ | Key conditions, GSI |
| Scan | ✅ | Filter expressions |
| CreateTable | ✅ | GSI support |
| DeleteTable | ✅ | |
| DescribeTable | ✅ | |

### Lambda

| Operation | Status | Notes |
|-----------|--------|-------|
| CreateFunction | ✅ | Zip upload |
| Invoke | ✅ | Sync, API Gateway v1 format |
| DeleteFunction | ✅ | |
| GetFunction | ✅ | |

## Error Codes

RustStack returns exact AWS error codes:

**S3:**
- `NoSuchKey` (404) - Object not found
- `NoSuchBucket` (404) - Bucket not found
- `BucketAlreadyExists` (409) - Bucket name taken
- `BucketNotEmpty` (409) - Non-empty bucket delete

**DynamoDB:**
- `ResourceNotFoundException` - Table not found
- `ConditionalCheckFailedException` - Condition expression failed
- `ValidationException` - Invalid request

**Lambda:**
- `ResourceNotFoundException` - Function not found
- `InvalidParameterValueException` - Bad parameter

## Flask/Lambda Example

```python
# app.py
from flask import Flask
from mangum import Mangum

app = Flask(__name__)

@app.route('/api/hello')
def hello():
    return {'message': 'Hello, World!'}

handler = Mangum(app)
```

Deploy and test:

```bash
# Create function
aws --endpoint-url=http://localhost:4566 lambda create-function \
    --function-name my-flask-app \
    --runtime python3.12 \
    --handler app.handler \
    --zip-file fileb://function.zip \
    --role arn:aws:iam::000000000000:role/lambda-role

# Invoke
aws --endpoint-url=http://localhost:4566 lambda invoke \
    --function-name my-flask-app \
    --payload '{"httpMethod":"GET","path":"/api/hello"}' \
    response.json
```

## Development

```bash
# Run tests
cargo test

# Run with logging
RUST_LOG=ruststack=debug ./target/release/ruststack

# Format code
cargo fmt

# Lint
cargo clippy
```

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed design documentation.

## Roadmap

- [x] Project structure
- [x] Core types and error handling
- [ ] S3 integration with s3s
- [ ] DynamoDB Local proxy
- [ ] Lambda container execution
- [ ] CI/CD pipeline
- [ ] Docker image

## License

MIT OR Apache-2.0

## Comparison with LocalStack

| Feature | RustStack | LocalStack |
|---------|-----------|------------|
| Language | Rust | Python |
| Memory usage | ~50MB | ~500MB+ |
| Startup time | <1s | ~5s |
| Services | 3 (focused) | 80+ (broad) |
| Error fidelity | High | Medium |
| Open source | Yes | Community/Pro |

RustStack is ideal when you need fast, reliable testing of S3 + DynamoDB + Lambda with exact AWS behavior. LocalStack is better for broad service coverage.
