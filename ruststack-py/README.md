# RustStack Python Bindings

In-process Python bindings for RustStack using pyo3.

## Installation

```bash
# Install build dependencies
pip install maturin pytest

# Build and install
cd ruststack-py
maturin develop

# Or install from PyPI (when published)
pip install ruststack-py
```

## Usage

```python
import ruststack_py

# Create RustStack instance
rs = ruststack_py.RustStack()

# DynamoDB
rs.ddb_create_table("users", "id", "S")
rs.ddb_put_item("users", "user1", '{"name": "John"}')
item = rs.ddb_get_item("users", "user1")

# S3
rs.s3_create_bucket("my-bucket")
rs.s3_put_object("my-bucket", "key.txt", "content")
obj = rs.s3_get_object("my-bucket", "key.txt")

# Secrets Manager
rs.secrets_create_secret("my-secret", "secret-value")
value = rs.secrets_get_secret_value("my-secret")

# Firehose
rs.firehose_create_delivery_stream("my-stream")
rs.firehose_put_record("my-stream", "data")

# IAM
rs.iam_create_role("my-role", '{"Statement": []}')

# SNS
rs.sns_create_topic("my-topic")
rs.sns_publish("my-topic", "message")

# SQS
rs.sqs_create_queue("my-queue")
rs.sqs_send_message("my-queue", "hello")
messages = rs.sqs_receive_message("my-queue", 10)
```

## Running Tests

```bash
# Build the extension first
cd ruststack-py
maturin develop

# Run in-process tests
pytest ../tests/integration/test_inprocess.py -v
```

## Why Use In-Process?

- **Faster**: No HTTP overhead, no container startup
- **Simpler**: No Docker dependency
- **Isolated**: Each test gets fresh state
- **Debugging**: Easy to inspect state in Python debugger
