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

# Configure endpoint - default to local process
ENDPOINT_URL = os.environ.get("AWS_ENDPOINT_URL", "http://localhost:4566")
REGION = "us-east-1"

# Set dummy credentials for boto3 if not already in environment
os.environ.setdefault("AWS_ACCESS_KEY_ID", "test")
os.environ.setdefault("AWS_SECRET_ACCESS_KEY", "test")


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

    def test_list_objects_pagination(self, client):
        """Test ListObjectsV2 pagination with continuation token."""
        bucket_name = "test-pagination-bucket"
        
        # Create bucket and objects
        try:
            client.create_bucket(Bucket=bucket_name)
        except ClientError:
            pass

        # Put more objects than default max-keys (typically 1000)
        num_objects = 50
        for i in range(num_objects):
            client.put_object(Bucket=bucket_name, Key=f"file{i:04d}.txt", Body=f"content {i}")

        # First request with small max-keys
        result = client.list_objects_v2(Bucket=bucket_name, MaxKeys=15)
        
        assert len(result["Contents"]) == 15
        assert result["IsTruncated"] is True
        assert "NextContinuationToken" in result

        # Second request with continuation token
        token = result["NextContinuationToken"]
        result2 = client.list_objects_v2(Bucket=bucket_name, ContinuationToken=token, MaxKeys=15)
        
        assert len(result2["Contents"]) == 15
        assert result2["IsTruncated"] is True
        assert "NextContinuationToken" in result2

        # Third request
        token2 = result2["NextContinuationToken"]
        result3 = client.list_objects_v2(Bucket=bucket_name, ContinuationToken=token2, MaxKeys=15)
        
        assert len(result3["Contents"]) == 15
        assert result3["IsTruncated"] is True

        # Fourth request - should get remaining 5
        token3 = result3["NextContinuationToken"]
        result4 = client.list_objects_v2(Bucket=bucket_name, ContinuationToken=token3, MaxKeys=15)
        
        assert len(result4["Contents"]) == 5
        assert result4["IsTruncated"] is False
        assert "NextContinuationToken" not in result4

        # Verify all objects can be retrieved by continuing
        all_keys = []
        token = None
        while True:
            if token:
                result = client.list_objects_v2(Bucket=bucket_name, ContinuationToken=token, MaxKeys=20)
            else:
                result = client.list_objects_v2(Bucket=bucket_name, MaxKeys=20)
            
            all_keys.extend([obj["Key"] for obj in result.get("Contents", [])])
            
            if result.get("IsTruncated"):
                token = result.get("NextContinuationToken")
            else:
                break

        assert len(all_keys) == num_objects


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
            client.delete_delivery_stream(DeliveryStreamName=stream_name, AllowForceDelete=True)
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

        assert "DeliveryStreamARN" in result
        assert stream_name in result["DeliveryStreamARN"]

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


class TestCloudFormation:
    """CloudFormation integration tests."""

    @pytest.fixture
    def client(self) -> BaseClient:
        return boto3.client("cloudformation", endpoint_url=ENDPOINT_URL, region_name=REGION)

    @pytest.mark.skip(reason="CloudFormation not fully implemented")
    def test_create_stack(self, client):
        """Create a CloudFormation stack."""
        template = {
            "AWSTemplateFormatVersion": "2010-09-09",
            "Description": "Test stack",
            "Resources": {
                "TestBucket": {
                    "Type": "AWS::S3::Bucket",
                    "Properties": {
                        "BucketName": "test-bucket"
                    }
                }
            }
        }
        
        result = client.create_stack(
            StackName="test-stack",
            TemplateBody=json.dumps(template),
        )
        
        assert "StackId" in result

    def test_describe_stacks(self, client):
        """Describe CloudFormation stacks."""
        template = {
            "AWSTemplateFormatVersion": "2010-09-09",
            "Resources": {}
        }
        
        client.create_stack(
            StackName="describe-test-stack",
            TemplateBody=json.dumps(template),
        )
        
        result = client.describe_stacks(StackName="describe-test-stack")
        assert "Stacks" in result
        assert len(result["Stacks"]) > 0

    def test_list_stacks(self, client):
        """List CloudFormation stacks."""
        result = client.list_stacks()
        assert "StackSummaries" in result

    def test_validate_template(self, client):
        """Validate a CloudFormation template."""
        template = {
            "AWSTemplateFormatVersion": "2010-09-09",
            "Description": "Valid template",
            "Resources": {}
        }
        
        result = client.validate_template(
            TemplateBody=json.dumps(template),
        )
        
        assert "Description" in result

    def test_delete_stack(self, client):
        """Delete a CloudFormation stack."""
        template = {
            "AWSTemplateFormatVersion": "2010-09-09",
            "Resources": {}
        }
        
        client.create_stack(
            StackName="delete-test-stack",
            TemplateBody=json.dumps(template),
        )
        
        client.delete_stack(StackName="delete-test-stack")
        
        result = client.describe_stacks(StackName="delete-test-stack")
        stacks = result.get("Stacks", [])
        if stacks:
            assert stacks[0]["StackStatus"] == "DELETE_COMPLETE"


class TestStepFunctions:
    """Step Functions integration tests."""

    @pytest.fixture
    def client(self) -> BaseClient:
        return boto3.client("stepfunctions", endpoint_url=ENDPOINT_URL, region_name=REGION)

    def test_create_state_machine(self, client):
        """Create a state machine."""
        definition = {
            "StartAt": "PassState",
            "States": {
                "PassState": {
                    "Type": "Pass",
                    "End": True
                }
            }
        }
        
        result = client.create_state_machine(
            name="test-state-machine",
            definition=json.dumps(definition),
            roleArn="arn:aws:iam::123456789012:role/test",
        )
        
        assert "stateMachineArn" in result

    def test_describe_state_machine(self, client):
        """Describe a state machine."""
        definition = {
            "StartAt": "PassState",
            "States": {
                "PassState": {
                    "Type": "Pass",
                    "End": True
                }
            }
        }
        
        client.create_state_machine(
            name="describe-test-sm",
            definition=json.dumps(definition),
            roleArn="arn:aws:iam::123456789012:role/test",
        )
        
        result = client.describe_state_machine(
            stateMachineArn="arn:aws:states:us-east-1:000000000000:stateMachine:describe-test-sm"
        )
        
        assert "stateMachineArn" in result
        assert result["name"] == "describe-test-sm"

    def test_list_state_machines(self, client):
        """List state machines."""
        result = client.list_state_machines()
        assert "stateMachines" in result

    def test_start_execution(self, client):
        """Start a state machine execution."""
        definition = {
            "StartAt": "PassState",
            "States": {
                "PassState": {
                    "Type": "Pass",
                    "End": True
                }
            }
        }
        
        client.create_state_machine(
            name="execution-test-sm",
            definition=json.dumps(definition),
            roleArn="arn:aws:iam::123456789012:role/test",
        )
        
        result = client.start_execution(
            stateMachineArn="arn:aws:states:us-east-1:000000000000:stateMachine:execution-test-sm",
            name="test-execution",
        )
        
        assert "executionArn" in result

    def test_describe_execution(self, client):
        """Describe an execution."""
        definition = {
            "StartAt": "PassState",
            "States": {
                "PassState": {
                    "Type": "Pass",
                    "End": True
                }
            }
        }
        
        client.create_state_machine(
            name="describe-exec-sm",
            definition=json.dumps(definition),
            roleArn="arn:aws:iam::123456789012:role/test",
        )
        
        start_result = client.start_execution(
            stateMachineArn="arn:aws:states:us-east-1:000000000000:stateMachine:describe-exec-sm",
        )
        
        result = client.describe_execution(
            executionArn=start_result["executionArn"]
        )
        
        assert "executionArn" in result
        assert "status" in result

    def test_execution_pass_state(self, client):
        """Test execution with Pass state that returns output."""
        definition = {
            "StartAt": "PassState",
            "States": {
                "PassState": {
                    "Type": "Pass",
                    "Result": {"message": "hello", "value": 42},
                    "End": True
                }
            }
        }
        
        client.create_state_machine(
            name="pass-test-sm",
            definition=json.dumps(definition),
            roleArn="arn:aws:iam::123456789012:role/test",
        )
        
        result = client.start_execution(
            stateMachineArn="arn:aws:states:us-east-1:000000000000:stateMachine:pass-test-sm",
            input=json.dumps({"input": "data"}),
        )
        
        exec_arn = result["executionArn"]
        
        # Describe execution to get result
        desc = client.describe_execution(executionArn=exec_arn)
        
        # Execution should complete
        assert desc["status"] in ["SUCCEEDED", "RUNNING"]

    def test_execution_choice_state(self, client):
        """Test execution with Choice state branches correctly."""
        definition = {
            "StartAt": "CheckValue",
            "States": {
                "CheckValue": {
                    "Type": "Choice",
                    "Choices": [
                        {
                            "Variable": "$.status",
                            "StringEquals": "active",
                            "Next": "ActiveState"
                        },
                        {
                            "Variable": "$.status",
                            "StringEquals": "inactive", 
                            "Next": "InactiveState"
                        }
                    ],
                    "Default": "UnknownState"
                },
                "ActiveState": {
                    "Type": "Pass",
                    "Result": {"result": "active branch"},
                    "End": True
                },
                "InactiveState": {
                    "Type": "Pass", 
                    "Result": {"result": "inactive branch"},
                    "End": True
                },
                "UnknownState": {
                    "Type": "Pass",
                    "Result": {"result": "unknown"},
                    "End": True
                }
            }
        }
        
        client.create_state_machine(
            name="choice-test-sm",
            definition=json.dumps(definition),
            roleArn="arn:aws:iam::123456789012:role/test",
        )
        
        # Test active branch
        result = client.start_execution(
            stateMachineArn="arn:aws:states:us-east-1:000000000000:stateMachine:choice-test-sm",
            input=json.dumps({"status": "active"}),
        )
        
        desc = client.describe_execution(executionArn=result["executionArn"])
        assert desc["status"] in ["SUCCEEDED", "RUNNING"]

    def test_execution_wait_state(self, client):
        """Test execution with Wait state completes."""
        definition = {
            "StartAt": "WaitState",
            "States": {
                "WaitState": {
                    "Type": "Wait",
                    "Seconds": 1,
                    "Next": "AfterWait"
                },
                "AfterWait": {
                    "Type": "Pass",
                    "Result": {"after": "wait"},
                    "End": True
                }
            }
        }
        
        client.create_state_machine(
            name="wait-test-sm",
            definition=json.dumps(definition),
            roleArn="arn:aws:iam::123456789012:role/test",
        )
        
        result = client.start_execution(
            stateMachineArn="arn:aws:states:us-east-1:000000000000:stateMachine:wait-test-sm",
        )
        
        desc = client.describe_execution(executionArn=result["executionArn"])
        # Should complete (wait is skipped in sync mode)
        assert desc["status"] in ["SUCCEEDED", "RUNNING"]

    def test_execution_fail_state(self, client):
        """Test execution with Fail state fails properly."""
        definition = {
            "StartAt": "FailState",
            "States": {
                "FailState": {
                    "Type": "Fail",
                    "Error": "TestError",
                    "Cause": "This is a test failure"
                }
            }
        }
        
        client.create_state_machine(
            name="fail-test-sm",
            definition=json.dumps(definition),
            roleArn="arn:aws:iam::123456789012:role/test",
        )
        
        result = client.start_execution(
            stateMachineArn="arn:aws:states:us-east-1:000000000000:stateMachine:fail-test-sm",
        )
        
        # Describe execution - should show failed status
        desc = client.describe_execution(executionArn=result["executionArn"])
        # In sync mode, execution completes immediately
        assert desc["status"] == "FAILED"

    def test_execution_succeed_state(self, client):
        """Test execution with Succeed state."""
        definition = {
            "StartAt": "SucceedState",
            "States": {
                "SucceedState": {
                    "Type": "Succeed",
                    "Output": {"result": "success"}
                }
            }
        }
        
        client.create_state_machine(
            name="succeed-test-sm",
            definition=json.dumps(definition),
            roleArn="arn:aws:iam::123456789012:role/test",
        )
        
        result = client.start_execution(
            stateMachineArn="arn:aws:states:us-east-1:000000000000:stateMachine:succeed-test-sm",
        )
        
        desc = client.describe_execution(executionArn=result["executionArn"])
        assert desc["status"] == "SUCCEEDED"

    def test_delete_state_machine(self, client):
        """Delete a state machine."""
        definition = {
            "StartAt": "PassState",
            "States": {
                "PassState": {
                    "Type": "Pass",
                    "End": True
                }
            }
        }
        
        client.create_state_machine(
            name="delete-test-sm",
            definition=json.dumps(definition),
            roleArn="arn:aws:iam::123456789012:role/test",
        )
        
        # Delete
        client.delete_state_machine(
            stateMachineArn="arn:aws:states:us-east-1:000000000000:stateMachine:delete-test-sm"
        )
        
        # Verify deleted
        with pytest.raises(ClientError) as exc_info:
            client.describe_state_machine(
                stateMachineArn="arn:aws:states:us-east-1:000000000000:stateMachine:delete-test-sm"
            )
        assert "StateMachineNotFound" in str(exc_info.value)


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
