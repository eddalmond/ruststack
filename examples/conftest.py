"""
RustStack pytest fixtures for integration testing.

Usage:
    1. Add ruststack to your docker-compose.yml (or use the provided one)
    2. Copy this file to your test directory
    3. Import fixtures in your tests

Example docker-compose.yml entry:

    services:
      ruststack:
        image: ghcr.io/eddalmond/ruststack:latest
        ports:
          - "4566:4566"
        healthcheck:
          test: ["CMD", "curl", "-f", "http://localhost:4566/health"]
          interval: 5s
          timeout: 3s
          retries: 5
"""

import os
import subprocess
import time
from typing import Generator

import boto3
import pytest
import requests


# Configuration
RUSTSTACK_PORT = int(os.environ.get("RUSTSTACK_PORT", "4566"))
RUSTSTACK_HOST = os.environ.get("RUSTSTACK_HOST", "localhost")
RUSTSTACK_ENDPOINT = f"http://{RUSTSTACK_HOST}:{RUSTSTACK_PORT}"

# Dummy AWS credentials (RustStack doesn't validate them)
AWS_ACCESS_KEY_ID = "testing"
AWS_SECRET_ACCESS_KEY = "testing"
AWS_REGION = "us-east-1"


def wait_for_ruststack(endpoint: str, timeout: float = 30.0) -> bool:
    """Wait for RustStack to be ready."""
    start = time.time()
    while time.time() - start < timeout:
        try:
            resp = requests.get(f"{endpoint}/health", timeout=1)
            if resp.status_code == 200:
                return True
        except requests.RequestException:
            pass
        time.sleep(0.1)
    return False


@pytest.fixture(scope="session")
def ruststack_endpoint() -> Generator[str, None, None]:
    """
    Session-scoped fixture that provides RustStack endpoint URL.
    
    Assumes RustStack is running via docker-compose.
    Start it with: docker-compose up -d ruststack
    """
    if not wait_for_ruststack(RUSTSTACK_ENDPOINT):
        pytest.skip(
            f"RustStack not available at {RUSTSTACK_ENDPOINT}. "
            "Start it with: docker-compose up -d ruststack"
        )
    yield RUSTSTACK_ENDPOINT


@pytest.fixture(scope="session")
def ruststack_process() -> Generator[str, None, None]:
    """
    Session-scoped fixture that starts RustStack as a subprocess.
    
    Use this if you have the binary available locally and don't want Docker.
    Set RUSTSTACK_BINARY env var to the path of the binary.
    """
    binary = os.environ.get("RUSTSTACK_BINARY", "./target/release/ruststack")
    
    if not os.path.exists(binary):
        pytest.skip(f"RustStack binary not found at {binary}")
    
    proc = subprocess.Popen(
        [binary, "--host", "0.0.0.0", "--port", str(RUSTSTACK_PORT)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    
    try:
        if not wait_for_ruststack(RUSTSTACK_ENDPOINT):
            proc.kill()
            stdout, stderr = proc.communicate()
            pytest.fail(
                f"RustStack failed to start.\n"
                f"stdout: {stdout.decode()}\n"
                f"stderr: {stderr.decode()}"
            )
        yield RUSTSTACK_ENDPOINT
    finally:
        proc.terminate()
        proc.wait(timeout=5)


# ============================================
# S3 Fixtures
# ============================================

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
    import uuid
    bucket_name = f"test-bucket-{uuid.uuid4().hex[:8]}"
    s3_client.create_bucket(Bucket=bucket_name)
    yield bucket_name
    # Cleanup: delete all objects then bucket
    try:
        response = s3_client.list_objects_v2(Bucket=bucket_name)
        for obj in response.get("Contents", []):
            s3_client.delete_object(Bucket=bucket_name, Key=obj["Key"])
        s3_client.delete_bucket(Bucket=bucket_name)
    except Exception:
        pass  # Best effort cleanup


# ============================================
# DynamoDB Fixtures
# ============================================

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


@pytest.fixture(scope="session")
def dynamodb_resource(ruststack_endpoint: str):
    """Session-scoped DynamoDB resource (higher-level API)."""
    return boto3.resource(
        "dynamodb",
        endpoint_url=ruststack_endpoint,
        aws_access_key_id=AWS_ACCESS_KEY_ID,
        aws_secret_access_key=AWS_SECRET_ACCESS_KEY,
        region_name=AWS_REGION,
    )


@pytest.fixture
def dynamodb_table(dynamodb_client) -> Generator[str, None, None]:
    """Function-scoped DynamoDB table with simple key schema."""
    import uuid
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


# ============================================
# Lambda Fixtures
# ============================================

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


# ============================================
# CloudWatch Logs Fixtures
# ============================================

@pytest.fixture(scope="session")
def logs_client(ruststack_endpoint: str):
    """Session-scoped CloudWatch Logs client."""
    return boto3.client(
        "logs",
        endpoint_url=ruststack_endpoint,
        aws_access_key_id=AWS_ACCESS_KEY_ID,
        aws_secret_access_key=AWS_SECRET_ACCESS_KEY,
        region_name=AWS_REGION,
    )


# ============================================
# Secrets Manager Fixtures
# ============================================

@pytest.fixture(scope="session")
def secretsmanager_client(ruststack_endpoint: str):
    """Session-scoped Secrets Manager client."""
    return boto3.client(
        "secretsmanager",
        endpoint_url=ruststack_endpoint,
        aws_access_key_id=AWS_ACCESS_KEY_ID,
        aws_secret_access_key=AWS_SECRET_ACCESS_KEY,
        region_name=AWS_REGION,
    )


@pytest.fixture
def secret(secretsmanager_client) -> Generator[str, None, None]:
    """Function-scoped secret (created and cleaned up per test)."""
    import uuid
    secret_name = f"test-secret-{uuid.uuid4().hex[:8]}"
    secretsmanager_client.create_secret(
        Name=secret_name,
        SecretString='{"username": "admin", "password": "secret123"}'
    )
    yield secret_name
    try:
        secretsmanager_client.delete_secret(
            SecretId=secret_name,
            ForceDeleteWithoutRecovery=True
        )
    except Exception:
        pass


# ============================================
# IAM Fixtures
# ============================================

@pytest.fixture(scope="session")
def iam_client(ruststack_endpoint: str):
    """Session-scoped IAM client."""
    return boto3.client(
        "iam",
        endpoint_url=ruststack_endpoint,
        aws_access_key_id=AWS_ACCESS_KEY_ID,
        aws_secret_access_key=AWS_SECRET_ACCESS_KEY,
        region_name=AWS_REGION,
    )


@pytest.fixture
def iam_role(iam_client) -> Generator[str, None, None]:
    """Function-scoped IAM role."""
    import uuid
    role_name = f"test-role-{uuid.uuid4().hex[:8]}"
    assume_role_policy = '''{
        "Version": "2012-10-17",
        "Statement": [{
            "Effect": "Allow",
            "Principal": {"Service": "lambda.amazonaws.com"},
            "Action": "sts:AssumeRole"
        }]
    }'''
    iam_client.create_role(
        RoleName=role_name,
        AssumeRolePolicyDocument=assume_role_policy
    )
    yield role_name
    try:
        # Detach all policies first
        attached = iam_client.list_attached_role_policies(RoleName=role_name)
        for policy in attached.get("AttachedPolicies", []):
            iam_client.detach_role_policy(
                RoleName=role_name,
                PolicyArn=policy["PolicyArn"]
            )
        iam_client.delete_role(RoleName=role_name)
    except Exception:
        pass


# ============================================
# API Gateway V2 Fixtures
# ============================================

@pytest.fixture(scope="session")
def apigatewayv2_client(ruststack_endpoint: str):
    """Session-scoped API Gateway V2 client."""
    return boto3.client(
        "apigatewayv2",
        endpoint_url=ruststack_endpoint,
        aws_access_key_id=AWS_ACCESS_KEY_ID,
        aws_secret_access_key=AWS_SECRET_ACCESS_KEY,
        region_name=AWS_REGION,
    )


@pytest.fixture
def http_api(apigatewayv2_client) -> Generator[str, None, None]:
    """Function-scoped HTTP API."""
    import uuid
    api_name = f"test-api-{uuid.uuid4().hex[:8]}"
    response = apigatewayv2_client.create_api(
        Name=api_name,
        ProtocolType="HTTP"
    )
    api_id = response["ApiId"]
    yield api_id
    try:
        apigatewayv2_client.delete_api(ApiId=api_id)
    except Exception:
        pass


# ============================================
# Kinesis Firehose Fixtures
# ============================================

@pytest.fixture(scope="session")
def firehose_client(ruststack_endpoint: str):
    """Session-scoped Kinesis Firehose client."""
    return boto3.client(
        "firehose",
        endpoint_url=ruststack_endpoint,
        aws_access_key_id=AWS_ACCESS_KEY_ID,
        aws_secret_access_key=AWS_SECRET_ACCESS_KEY,
        region_name=AWS_REGION,
    )


@pytest.fixture
def delivery_stream(firehose_client, s3_bucket) -> Generator[str, None, None]:
    """Function-scoped Firehose delivery stream."""
    import uuid
    stream_name = f"test-stream-{uuid.uuid4().hex[:8]}"
    firehose_client.create_delivery_stream(
        DeliveryStreamName=stream_name,
        DeliveryStreamType="DirectPut",
        ExtendedS3DestinationConfiguration={
            "BucketARN": f"arn:aws:s3:::{s3_bucket}",
            "RoleARN": "arn:aws:iam::000000000000:role/firehose-role",
            "BufferingHints": {
                "SizeInMBs": 1,
                "IntervalInSeconds": 60
            }
        }
    )
    yield stream_name
    try:
        firehose_client.delete_delivery_stream(DeliveryStreamName=stream_name)
    except Exception:
        pass
