#!/usr/bin/env python3
"""
Integration tests for RustStack.

Tests S3, DynamoDB, and Lambda APIs against a running RustStack instance.
"""

import json
import sys
import time
import uuid

import boto3
import requests
from botocore.config import Config

ENDPOINT_URL = "http://localhost:4566"

# Configure boto3 client
config = Config(
    retries={"max_attempts": 3, "mode": "standard"},
    connect_timeout=5,
    read_timeout=30,
)


def get_client(service: str):
    """Create a boto3 client for the given service."""
    return boto3.client(
        service,
        endpoint_url=ENDPOINT_URL,
        aws_access_key_id="test",
        aws_secret_access_key="test",
        region_name="us-east-1",
        config=config,
    )


def test_health_check():
    """Test the health check endpoint."""
    print("Testing health check...")

    response = requests.get(f"{ENDPOINT_URL}/health", timeout=10)
    assert response.status_code == 200, f"Health check failed: {response.status_code}"

    data = response.json()
    assert data.get("status") == "running", f"Unexpected status: {data}"

    print("✓ Health check passed")


def test_s3_operations():
    """Test S3 bucket and object operations."""
    print("Testing S3 operations...")

    s3 = get_client("s3")
    bucket_name = f"test-bucket-{uuid.uuid4().hex[:8]}"
    object_key = "test-object.txt"
    object_content = b"Hello, RustStack!"

    try:
        # Create bucket
        s3.create_bucket(Bucket=bucket_name)
        print(f"  Created bucket: {bucket_name}")

        # List buckets
        buckets = s3.list_buckets()
        bucket_names = [b["Name"] for b in buckets["Buckets"]]
        assert bucket_name in bucket_names, f"Bucket not found in list: {bucket_names}"
        print(f"  Listed buckets: {len(bucket_names)} buckets")

        # Put object
        s3.put_object(Bucket=bucket_name, Key=object_key, Body=object_content)
        print(f"  Put object: {object_key}")

        # Get object
        response = s3.get_object(Bucket=bucket_name, Key=object_key)
        retrieved_content = response["Body"].read()
        assert retrieved_content == object_content, "Object content mismatch"
        print(f"  Got object: {len(retrieved_content)} bytes")

        # List objects
        response = s3.list_objects_v2(Bucket=bucket_name)
        objects = response.get("Contents", [])
        assert len(objects) == 1, f"Expected 1 object, got {len(objects)}"
        assert objects[0]["Key"] == object_key
        print(f"  Listed objects: {len(objects)} objects")

        # Delete object
        s3.delete_object(Bucket=bucket_name, Key=object_key)
        print(f"  Deleted object: {object_key}")

        # Delete bucket
        s3.delete_bucket(Bucket=bucket_name)
        print(f"  Deleted bucket: {bucket_name}")

    except Exception as e:
        # Cleanup on failure
        try:
            s3.delete_object(Bucket=bucket_name, Key=object_key)
        except:
            pass
        try:
            s3.delete_bucket(Bucket=bucket_name)
        except:
            pass
        raise e

    print("✓ S3 operations passed")


def test_dynamodb_operations():
    """Test DynamoDB table and item operations."""
    print("Testing DynamoDB operations...")

    dynamodb = get_client("dynamodb")
    table_name = f"test-table-{uuid.uuid4().hex[:8]}"

    try:
        # Create table
        dynamodb.create_table(
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
        print(f"  Created table: {table_name}")

        # Wait for table to be active (RustStack is fast, but let's be safe)
        time.sleep(0.5)

        # Describe table
        response = dynamodb.describe_table(TableName=table_name)
        status = response["Table"]["TableStatus"]
        assert status == "ACTIVE", f"Table not active: {status}"
        print(f"  Table status: {status}")

        # Put item
        item = {
            "pk": {"S": "user#123"},
            "sk": {"S": "profile"},
            "name": {"S": "Test User"},
            "email": {"S": "test@example.com"},
            "age": {"N": "25"},
        }
        dynamodb.put_item(TableName=table_name, Item=item)
        print("  Put item")

        # Get item
        response = dynamodb.get_item(
            TableName=table_name,
            Key={"pk": {"S": "user#123"}, "sk": {"S": "profile"}},
        )
        retrieved_item = response.get("Item")
        assert retrieved_item is not None, "Item not found"
        assert retrieved_item["name"]["S"] == "Test User"
        print("  Got item")

        # Query
        response = dynamodb.query(
            TableName=table_name,
            KeyConditionExpression="pk = :pk",
            ExpressionAttributeValues={":pk": {"S": "user#123"}},
        )
        items = response.get("Items", [])
        assert len(items) == 1, f"Expected 1 item, got {len(items)}"
        print(f"  Query returned {len(items)} items")

        # Update item
        dynamodb.update_item(
            TableName=table_name,
            Key={"pk": {"S": "user#123"}, "sk": {"S": "profile"}},
            UpdateExpression="SET age = :age",
            ExpressionAttributeValues={":age": {"N": "26"}},
        )
        print("  Updated item")

        # Verify update
        response = dynamodb.get_item(
            TableName=table_name,
            Key={"pk": {"S": "user#123"}, "sk": {"S": "profile"}},
        )
        assert response["Item"]["age"]["N"] == "26", "Update failed"
        print("  Verified update")

        # Delete item
        dynamodb.delete_item(
            TableName=table_name,
            Key={"pk": {"S": "user#123"}, "sk": {"S": "profile"}},
        )
        print("  Deleted item")

        # Delete table
        dynamodb.delete_table(TableName=table_name)
        print(f"  Deleted table: {table_name}")

    except Exception as e:
        # Cleanup on failure
        try:
            dynamodb.delete_table(TableName=table_name)
        except:
            pass
        raise e

    print("✓ DynamoDB operations passed")


def test_lambda_api():
    """Test Lambda API responds (basic endpoint check)."""
    print("Testing Lambda API...")

    lambda_client = get_client("lambda")

    # List functions (should return empty list, but endpoint works)
    response = lambda_client.list_functions()
    functions = response.get("Functions", [])
    print(f"  Listed functions: {len(functions)} functions")

    print("✓ Lambda API passed")


def main():
    """Run all integration tests."""
    print("=" * 60)
    print("RustStack Integration Tests")
    print("=" * 60)
    print(f"Endpoint: {ENDPOINT_URL}")
    print()

    tests = [
        test_health_check,
        test_s3_operations,
        test_dynamodb_operations,
        test_lambda_api,
    ]

    failed = []

    for test in tests:
        try:
            test()
            print()
        except Exception as e:
            print(f"✗ {test.__name__} FAILED: {e}")
            print()
            failed.append((test.__name__, str(e)))

    print("=" * 60)

    if failed:
        print(f"FAILED: {len(failed)}/{len(tests)} tests")
        for name, error in failed:
            print(f"  - {name}: {error}")
        sys.exit(1)
    else:
        print(f"PASSED: {len(tests)}/{len(tests)} tests")
        sys.exit(0)


if __name__ == "__main__":
    main()
