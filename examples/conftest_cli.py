"""
Example pytest fixtures using ruststack-cli.

This file demonstrates how to use the ruststack-cli package to automatically
install and manage RustStack for your tests.

Usage:
    1. Add ruststack-cli to your project (poetry/uv/pip)
    2. Copy this file to your tests/ directory
    3. Use the fixtures in your tests

Example:

    poetry add --dev ruststack-cli

Then in your tests:

    def test_s3_bucket(s3_client):
        s3_client.create_bucket(Bucket="my-bucket")
        response = s3_client.list_buckets()
        assert "my-bucket" in [b["Name"] for b in response["Buckets"]]
"""

import uuid
from typing import Generator

import boto3
import pytest


AWS_ACCESS_KEY_ID = "test"
AWS_SECRET_ACCESS_KEY = "test"
AWS_REGION = "us-east-1"


@pytest.fixture(scope="session")
def ruststack_endpoint() -> Generator[str, None, None]:
    """
    Start RustStack for the test session.

    This fixture automatically downloads and installs the RustStack binary
    if needed, then starts the server.
    """
    from ruststack_cli.server import RustStackProcess

    process = RustStackProcess()
    process.start(wait=True)
    yield process.endpoint
    process.stop()


@pytest.fixture
def ruststack(ruststack_endpoint: str) -> str:
    """Alias for ruststack_endpoint."""
    return ruststack_endpoint


@pytest.fixture(scope="session")
def s3_client(ruststack_endpoint: str):
    """Session-scoped S3 client."""
    return boto3.client(
        "s3",
        endpoint_url=ruststack_endpoint,
        aws_access_key_id=AWS_ACCESS_KEY_ID,
        aws_secret_access_key=AWS_SECRET_ACCESS_KEY,
        region_name=AWS_REGION,
    )


@pytest.fixture
def s3_bucket(s3_client) -> Generator[str, None, None]:
    """Function-scoped S3 bucket (created and cleaned up per test)."""
    bucket_name = f"test-bucket-{uuid.uuid4().hex[:8]}"
    s3_client.create_bucket(Bucket=bucket_name)
    yield bucket_name
    try:
        response = s3_client.list_objects_v2(Bucket=bucket_name)
        for obj in response.get("Contents", []):
            s3_client.delete_object(Bucket=bucket_name, Key=obj["Key"])
        s3_client.delete_bucket(Bucket=bucket_name)
    except Exception:
        pass


@pytest.fixture(scope="session")
def dynamodb_client(ruststack_endpoint: str):
    """Session-scoped DynamoDB client."""
    return boto3.client(
        "dynamodb",
        endpoint_url=ruststack_endpoint,
        aws_access_key_id=AWS_ACCESS_KEY_ID,
        aws_secret_access_key=AWS_SECRET_ACCESS_KEY,
        region_name=AWS_REGION,
    )


@pytest.fixture
def dynamodb_table(dynamodb_client) -> Generator[str, None, None]:
    """Function-scoped DynamoDB table."""
    table_name = f"test-table-{uuid.uuid4().hex[:8]}"

    dynamodb_client.create_table(
        TableName=table_name,
        KeySchema=[
            {"AttributeName": "pk", "KeyType": "HASH"},
            {"AttributeName": "sk", "KeyType": "RANGE"},
        ],
        AttributeDefinitions=[
            {"AttributeName": "pk", "AttributeType": "S"},
            {"AttributeName": "sk", "AttributeType": "S"},
        ],
        BillingMode="PAY_PER_REQUEST",
    )
    yield table_name

    try:
        dynamodb_client.delete_table(TableName=table_name)
    except Exception:
        pass


@pytest.fixture(scope="session")
def lambda_client(ruststack_endpoint: str):
    """Session-scoped Lambda client."""
    return boto3.client(
        "lambda",
        endpoint_url=ruststack_endpoint,
        aws_access_key_id=AWS_ACCESS_KEY_ID,
        aws_secret_access_key=AWS_SECRET_ACCESS_KEY,
        region_name=AWS_REGION,
    )


@pytest.fixture(scope="session")
def sqs_client(ruststack_endpoint: str):
    """Session-scoped SQS client."""
    return boto3.client(
        "sqs",
        endpoint_url=ruststack_endpoint,
        aws_access_key_id=AWS_ACCESS_KEY_ID,
        aws_secret_access_key=AWS_SECRET_ACCESS_KEY,
        region_name=AWS_REGION,
    )


@pytest.fixture(scope="session")
def sns_client(ruststack_endpoint: str):
    """Session-scoped SNS client."""
    return boto3.client(
        "sns",
        endpoint_url=ruststack_endpoint,
        aws_access_key_id=AWS_ACCESS_KEY_ID,
        aws_secret_access_key=AWS_SECRET_ACCESS_KEY,
        region_name=AWS_REGION,
    )


@pytest.fixture(autouse=True)
def reset_state(ruststack_endpoint: str) -> Generator[None, None, None]:
    """Reset RustStack state between tests."""
    yield
    try:
        import requests

        requests.post(f"{ruststack_endpoint}/_reset", timeout=2)
    except Exception:
        pass
