"""
Integration tests for RustStack via Docker container.

These tests verify RustStack works as a drop-in replacement for LocalStack/moto.

Prerequisites:
    - Docker running
    - RustStack container: docker pull ghcr.io/eddalmond/ruststack:latest
    - Or build locally: cargo build --release

Usage:
    # Run tests against Docker container
    pytest tests/integration/test_docker.py -v

    # Run against local ruststack binary
    # Start ruststack: ./target/release/ruststack --port 4566
    # Then: AWS_ENDPOINT_URL=http://localhost:4566 pytest tests/integration/test_docker.py -v
"""
import json
import os
import time

import boto3
import pytest
import requests
from botocore.client import BaseClient
from botocore.exceptions import ClientError

# Configure endpoint - default to docker service name
ENDPOINT_URL = os.environ.get("AWS_ENDPOINT_URL", "http://ruststack:4566")
REGION = "us-east-1"


class TestRustStackHealth:
    """Basic health check tests."""

    def test_health_endpoint(self):
        """Health endpoint returns 200."""
        response = requests.get(f"{ENDPOINT_URL}/health", timeout=5)
        assert response.status_code == 200

    def test_localstack_health_endpoint(self):
        """LocalStack-compatible health endpoint."""
        response = requests.get(f"{ENDPOINT_URL}/_localstack/health", timeout=5)
        assert response.status_code == 200


class TestDynamoDB:
    """DynamoDB integration tests."""

    @pytest.fixture
    def client(self) -> BaseClient:
        return boto3.client("dynamodb", endpoint_url=ENDPOINT_URL, region_name=REGION)

    @pytest.fixture
    def resource(self):
        return boto3.resource("dynamodb", endpoint_url=ENDPOINT_URL, region_name=REGION)

    def test_create_table(self, client):
        """Create a DynamoDB table."""
        table_name = "test-table"
        
        # Clean up if exists
        try:
            client.delete_table(TableName=table_name)
            time.sleep(0.5)
        except ClientError:
            pass

        result = client.create_table(
            TableName=table_name,
            KeySchema=[
                {"AttributeName": "id", "KeyType": "HASH"},
            ],
            AttributeDefinitions=[
                {"AttributeName": "id", "AttributeType": "S"},
            ],
            ProvisionedThroughput={
                "ReadCapacityUnits": 5,
                "WriteCapacityUnits": 5,
            },
        )

        assert result["TableDescription"]["TableName"] == table_name
        assert result["TableDescription"]["TableStatus"] == "ACTIVE"

    def test_put_and_get_item(self, client, resource):
        """Put and get an item."""
        table_name = "test-items"
        
        # Create table
        try:
            client.create_table(
                TableName=table_name,
                KeySchema=[{"AttributeName": "id", "KeyType": "HASH"}],
                AttributeDefinitions=[{"AttributeName": "id", "AttributeType": "S"}],
                ProvisionedThroughput={"ReadCapacityUnits": 5, "WriteCapacityUnits": 5},
            )
            time.sleep(1)
        except ClientError as e:
            if "ResourceNotFoundException" not in str(e):
                raise

        # Put item
        client.put_item(
            TableName=table_name,
            Item={"id": {"S": "user1"}, "name": {"S": "John"}, "age": {"N": "30"}},
        )

        # Get item
        result = client.get_item(TableName=table_name, Key={"id": {"S": "user1"}})

        assert result["Item"]["id"]["S"] == "user1"
        assert result["Item"]["name"]["S"] == "John"
        assert result["Item"]["age"]["N"] == "30"

    def test_list_tables(self, client):
        """List tables."""
        result = client.list_tables()
        assert "TableNames" in result


class TestS3:
    """S3 integration tests."""

    @pytest.fixture
    def client(self) -> BaseClient:
        return boto3.client("s3", endpoint_url=ENDPOINT_URL, region_name=REGION)

    @pytest.fixture
    def resource(self):
        return boto3.resource("s3", endpoint_url=ENDPOINT_URL, region_name=REGION)

    def test_create_bucket(self, client):
        """Create an S3 bucket."""
        bucket_name = "test-bucket"
        
        # Delete if exists
        try:
            client.delete_bucket(Bucket=bucket_name)
        except ClientError:
            pass

        client.create_bucket(Bucket=bucket_name)

        # Verify exists
        result = client.list_buckets()
        bucket_names = [b["Name"] for b in result["Buckets"]]
        assert bucket_name in bucket_names

    def test_put_and_get_object(self, client):
        """Put and get an object."""
        bucket_name = "test-objects-bucket"
        key = "test/key.json"
        content = {"message": "hello", "number": 42}

        # Create bucket
        try:
            client.create_bucket(Bucket=bucket_name)
        except ClientError:
            pass

        # Put object
        client.put_object(
            Bucket=bucket_name,
            Key=key,
            Body=json.dumps(content),
            ContentType="application/json",
        )

        # Get object
        result = client.get_object(Bucket=bucket_name, Key=key)
        retrieved = json.loads(result["Body"].read())

        assert retrieved == content

    def test_list_objects(self, client):
        """List objects in bucket."""
        bucket_name = "test-list-bucket"
        
        # Create bucket and objects
        try:
            client.create_bucket(Bucket=bucket_name)
        except ClientError:
            pass

        for i in range(3):
            client.put_object(Bucket=bucket_name, Key=f"file{i}.txt", Body=f"content {i}")

        result = client.list_objects_v2(Bucket=bucket_name)
        assert len(result["Contents"]) == 3


class TestSecretsManager:
    """Secrets Manager integration tests."""

    @pytest.fixture
    def client(self) -> BaseClient:
        return boto3.client("secretsmanager", endpoint_url=ENDPOINT_URL, region_name=REGION)

    def test_create_secret(self, client):
        """Create a secret."""
        secret_name = "test-secret"
        
        # Delete if exists
        try:
            client.delete_secret(SecretId=secret_name, ForceDeleteWithoutRecovery=True)
        except ClientError:
            pass

        result = client.create_secret(
            Name=secret_name,
            SecretString="super-secret-value",
        )

        assert result["Name"] == secret_name

    def test_get_secret_value(self, client):
        """Get secret value."""
        secret_name = "test-get-secret"
        
        client.create_secret(Name=secret_name, SecretString="my-secret")

        result = client.get_secret_value(SecretId=secret_name)

        assert result["SecretString"] == "my-secret"

    def test_update_secret(self, client):
        """Update secret value."""
        secret_name = "test-update-secret"
        
        client.create_secret(Name=secret_name, SecretString="v1")
        client.put_secret_value(SecretId=secret_name, SecretString="v2")

        result = client.get_secret_value(SecretId=secret_name)
        assert result["SecretString"] == "v2"


class TestFirehose:
    """Firehose integration tests."""

    @pytest.fixture
    def client(self) -> BaseClient:
        return boto3.client("firehose", endpoint_url=ENDPOINT_URL, region_name=REGION)

    @pytest.fixture
    def s3_client(self) -> BaseClient:
        return boto3.client("s3", endpoint_url=ENDPOINT_URL, region_name=REGION)

    def test_create_delivery_stream(self, client, s3_client):
        """Create a Firehose delivery stream."""
        stream_name = "test-firehose-stream"
        bucket_name = "test-firehose-bucket"

        # Create S3 bucket first
        try:
            s3_client.create_bucket(Bucket=bucket_name)
        except ClientError:
            pass

        # Delete stream if exists
        try:
            client.delete_delivery_stream(DeliveryStreamName=stream_name, ForceDelete=True)
            time.sleep(1)
        except ClientError:
            pass

        result = client.create_delivery_stream(
            DeliveryStreamName=stream_name,
            DeliveryStreamType="DirectPut",
            ExtendedS3DestinationConfiguration={
                "BucketARN": f"arn:aws:s3:::{bucket_name}",
                "RoleARN": "arn:aws:iam::123456789012:role/test-role",
                "Prefix": "firehose/",
            },
        )

        assert result["DeliveryStreamDescription"]["DeliveryStreamName"] == stream_name

    def test_put_record(self, client):
        """Put a record to Firehose."""
        stream_name = "test-firehose-record"
        
        # Create stream (or use existing)
        try:
            client.create_delivery_stream(
                DeliveryStreamName=stream_name,
                DeliveryStreamType="DirectPut",
                S3DestinationConfiguration={
                    "BucketARN": "arn:aws:s3:::test-bucket",
                    "RoleARN": "arn:aws:iam::123456789012:role/test-role",
                },
            )
            time.sleep(1)
        except ClientError:
            pass

        result = client.put_record(
            DeliveryStreamName=stream_name,
            Record={"Data": b"test record data"},
        )

        assert "RecordId" in result


class TestIAM:
    """IAM integration tests."""

    @pytest.fixture
    def client(self) -> BaseClient:
        return boto3.client("iam", endpoint_url=ENDPOINT_URL, region_name=REGION)

    def test_create_role(self, client):
        """Create an IAM role."""
        role_name = "test-role"
        policy = json.dumps({
            "Version": "2012-10-17",
            "Statement": [{
                "Effect": "Allow",
                "Action": "s3:*",
                "Resource": "*"
            }]
        })

        result = client.create_role(
            RoleName=role_name,
            AssumeRolePolicyDocument=policy,
        )

        assert result["Role"]["RoleName"] == role_name

    def test_list_roles(self, client):
        """List IAM roles."""
        result = client.list_roles()
        assert "Roles" in result


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
