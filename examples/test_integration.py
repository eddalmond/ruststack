"""
RustStack Integration Tests

Run with:
    # Start RustStack first
    cargo run --release -- --host 0.0.0.0 --port 4566

    # Then run tests
    cd examples && pytest test_integration.py -v
"""

import base64
import json
import uuid

import pytest


# ============================================
# S3 Tests
# ============================================

class TestS3:
    def test_bucket_lifecycle(self, s3_client):
        """Test create, list, and delete bucket."""
        bucket_name = f"test-{uuid.uuid4().hex[:8]}"

        # Create
        s3_client.create_bucket(Bucket=bucket_name)

        # List
        response = s3_client.list_buckets()
        bucket_names = [b["Name"] for b in response["Buckets"]]
        assert bucket_name in bucket_names

        # Delete
        s3_client.delete_bucket(Bucket=bucket_name)

        # Verify deleted
        response = s3_client.list_buckets()
        bucket_names = [b["Name"] for b in response["Buckets"]]
        assert bucket_name not in bucket_names

    def test_object_crud(self, s3_client, s3_bucket):
        """Test put, get, and delete object."""
        key = "test-object.txt"
        body = b"Hello, RustStack!"

        # Put
        s3_client.put_object(Bucket=s3_bucket, Key=key, Body=body)

        # Get
        response = s3_client.get_object(Bucket=s3_bucket, Key=key)
        assert response["Body"].read() == body

        # Delete
        s3_client.delete_object(Bucket=s3_bucket, Key=key)

        # Verify deleted
        with pytest.raises(s3_client.exceptions.NoSuchKey):
            s3_client.get_object(Bucket=s3_bucket, Key=key)

    def test_list_objects_with_prefix(self, s3_client, s3_bucket):
        """Test listing objects with prefix filter."""
        # Create objects with different prefixes
        s3_client.put_object(Bucket=s3_bucket, Key="docs/file1.txt", Body=b"1")
        s3_client.put_object(Bucket=s3_bucket, Key="docs/file2.txt", Body=b"2")
        s3_client.put_object(Bucket=s3_bucket, Key="images/pic.png", Body=b"3")

        # List with prefix
        response = s3_client.list_objects_v2(Bucket=s3_bucket, Prefix="docs/")
        keys = [obj["Key"] for obj in response.get("Contents", [])]

        assert "docs/file1.txt" in keys
        assert "docs/file2.txt" in keys
        assert "images/pic.png" not in keys


# ============================================
# DynamoDB Tests
# ============================================

class TestDynamoDB:
    def test_table_lifecycle(self, dynamodb_client):
        """Test create, describe, and delete table."""
        table_name = f"test-{uuid.uuid4().hex[:8]}"

        # Create
        dynamodb_client.create_table(
            TableName=table_name,
            KeySchema=[{"AttributeName": "id", "KeyType": "HASH"}],
            AttributeDefinitions=[{"AttributeName": "id", "AttributeType": "S"}],
            BillingMode="PAY_PER_REQUEST",
        )

        # Describe
        response = dynamodb_client.describe_table(TableName=table_name)
        assert response["Table"]["TableName"] == table_name

        # Delete
        dynamodb_client.delete_table(TableName=table_name)

    def test_item_crud(self, dynamodb_client, dynamodb_table):
        """Test put, get, update, and delete item."""
        item = {"pk": {"S": "user#123"}, "sk": {"S": "profile"}, "name": {"S": "Alice"}}

        # Put
        dynamodb_client.put_item(TableName=dynamodb_table, Item=item)

        # Get
        response = dynamodb_client.get_item(
            TableName=dynamodb_table,
            Key={"pk": {"S": "user#123"}, "sk": {"S": "profile"}}
        )
        assert response["Item"]["name"]["S"] == "Alice"

        # Update
        dynamodb_client.update_item(
            TableName=dynamodb_table,
            Key={"pk": {"S": "user#123"}, "sk": {"S": "profile"}},
            UpdateExpression="SET #n = :name",
            ExpressionAttributeNames={"#n": "name"},
            ExpressionAttributeValues={":name": {"S": "Bob"}}
        )

        response = dynamodb_client.get_item(
            TableName=dynamodb_table,
            Key={"pk": {"S": "user#123"}, "sk": {"S": "profile"}}
        )
        assert response["Item"]["name"]["S"] == "Bob"

        # Delete
        dynamodb_client.delete_item(
            TableName=dynamodb_table,
            Key={"pk": {"S": "user#123"}, "sk": {"S": "profile"}}
        )

    def test_query_with_conditions(self, dynamodb_client, dynamodb_table):
        """Test query with key conditions and filters."""
        # Insert test data
        for i in range(5):
            dynamodb_client.put_item(
                TableName=dynamodb_table,
                Item={
                    "pk": {"S": "order#100"},
                    "sk": {"S": f"item#{i:03d}"},
                    "price": {"N": str(i * 10)}
                }
            )

        # Query with range condition
        response = dynamodb_client.query(
            TableName=dynamodb_table,
            KeyConditionExpression="pk = :pk AND sk BETWEEN :start AND :end",
            ExpressionAttributeValues={
                ":pk": {"S": "order#100"},
                ":start": {"S": "item#001"},
                ":end": {"S": "item#003"}
            }
        )

        assert response["Count"] == 3


# ============================================
# Secrets Manager Tests
# ============================================

class TestSecretsManager:
    def test_secret_lifecycle(self, secretsmanager_client):
        """Test create, get, update, and delete secret."""
        secret_name = f"test-{uuid.uuid4().hex[:8]}"

        # Create
        secretsmanager_client.create_secret(
            Name=secret_name,
            SecretString='{"api_key": "abc123"}'
        )

        # Get
        response = secretsmanager_client.get_secret_value(SecretId=secret_name)
        assert json.loads(response["SecretString"])["api_key"] == "abc123"

        # Update (put new version)
        secretsmanager_client.put_secret_value(
            SecretId=secret_name,
            SecretString='{"api_key": "xyz789"}'
        )

        response = secretsmanager_client.get_secret_value(SecretId=secret_name)
        assert json.loads(response["SecretString"])["api_key"] == "xyz789"

        # Delete
        secretsmanager_client.delete_secret(
            SecretId=secret_name,
            ForceDeleteWithoutRecovery=True
        )

    def test_secret_versioning(self, secretsmanager_client, secret):
        """Test that secret versions are tracked."""
        # Get original version
        v1 = secretsmanager_client.get_secret_value(SecretId=secret)
        v1_id = v1["VersionId"]

        # Put new version
        secretsmanager_client.put_secret_value(
            SecretId=secret,
            SecretString='{"username": "admin", "password": "newpassword"}'
        )

        # Current version should be different
        v2 = secretsmanager_client.get_secret_value(SecretId=secret)
        assert v2["VersionId"] != v1_id

        # Should be able to get by version stage
        assert "AWSCURRENT" in v2["VersionStages"]


# ============================================
# IAM Tests
# ============================================

class TestIAM:
    def test_role_lifecycle(self, iam_client):
        """Test create, get, and delete role."""
        role_name = f"test-{uuid.uuid4().hex[:8]}"
        assume_role_policy = json.dumps({
            "Version": "2012-10-17",
            "Statement": [{
                "Effect": "Allow",
                "Principal": {"Service": "ec2.amazonaws.com"},
                "Action": "sts:AssumeRole"
            }]
        })

        # Create
        response = iam_client.create_role(
            RoleName=role_name,
            AssumeRolePolicyDocument=assume_role_policy
        )
        assert response["Role"]["RoleName"] == role_name

        # Get
        response = iam_client.get_role(RoleName=role_name)
        assert response["Role"]["RoleName"] == role_name

        # Delete
        iam_client.delete_role(RoleName=role_name)

    def test_policy_attachment(self, iam_client, iam_role):
        """Test attach and detach policy from role."""
        # Create policy
        policy_name = f"test-policy-{uuid.uuid4().hex[:8]}"
        policy_doc = json.dumps({
            "Version": "2012-10-17",
            "Statement": [{
                "Effect": "Allow",
                "Action": "s3:GetObject",
                "Resource": "*"
            }]
        })

        policy_response = iam_client.create_policy(
            PolicyName=policy_name,
            PolicyDocument=policy_doc
        )
        policy_arn = policy_response["Policy"]["Arn"]

        try:
            # Attach
            iam_client.attach_role_policy(RoleName=iam_role, PolicyArn=policy_arn)

            # List attached
            response = iam_client.list_attached_role_policies(RoleName=iam_role)
            arns = [p["PolicyArn"] for p in response["AttachedPolicies"]]
            assert policy_arn in arns

            # Detach
            iam_client.detach_role_policy(RoleName=iam_role, PolicyArn=policy_arn)
        finally:
            iam_client.delete_policy(PolicyArn=policy_arn)


# ============================================
# API Gateway V2 Tests
# ============================================

class TestAPIGatewayV2:
    def test_api_lifecycle(self, apigatewayv2_client):
        """Test create, get, and delete API."""
        api_name = f"test-{uuid.uuid4().hex[:8]}"

        # Create
        response = apigatewayv2_client.create_api(Name=api_name, ProtocolType="HTTP")
        api_id = response["ApiId"]

        try:
            assert response["Name"] == api_name
            assert response["ProtocolType"] == "HTTP"

            # Get
            response = apigatewayv2_client.get_api(ApiId=api_id)
            assert response["Name"] == api_name
        finally:
            # Delete
            apigatewayv2_client.delete_api(ApiId=api_id)

    def test_route_and_integration(self, apigatewayv2_client, http_api):
        """Test creating routes and integrations."""
        # Create integration
        int_response = apigatewayv2_client.create_integration(
            ApiId=http_api,
            IntegrationType="AWS_PROXY",
            IntegrationUri="arn:aws:lambda:us-east-1:000000000000:function:test",
            PayloadFormatVersion="2.0"
        )
        integration_id = int_response["IntegrationId"]

        # Create route
        route_response = apigatewayv2_client.create_route(
            ApiId=http_api,
            RouteKey="GET /test",
            Target=f"integrations/{integration_id}"
        )

        assert route_response["RouteKey"] == "GET /test"

        # List routes
        routes = apigatewayv2_client.get_routes(ApiId=http_api)
        route_keys = [r["RouteKey"] for r in routes["Items"]]
        assert "GET /test" in route_keys

    def test_stage_management(self, apigatewayv2_client, http_api):
        """Test stage creation and management."""
        # Create stage
        response = apigatewayv2_client.create_stage(
            ApiId=http_api,
            StageName="prod",
            AutoDeploy=True
        )

        assert response["StageName"] == "prod"
        assert response["AutoDeploy"] is True

        # Get stage
        response = apigatewayv2_client.get_stage(ApiId=http_api, StageName="prod")
        assert response["StageName"] == "prod"

        # List stages
        stages = apigatewayv2_client.get_stages(ApiId=http_api)
        stage_names = [s["StageName"] for s in stages["Items"]]
        assert "prod" in stage_names


# ============================================
# Kinesis Firehose Tests
# ============================================

class TestFirehose:
    def test_delivery_stream_lifecycle(self, firehose_client, s3_bucket):
        """Test create, describe, and delete delivery stream."""
        stream_name = f"test-{uuid.uuid4().hex[:8]}"

        # Create
        firehose_client.create_delivery_stream(
            DeliveryStreamName=stream_name,
            DeliveryStreamType="DirectPut",
            ExtendedS3DestinationConfiguration={
                "BucketARN": f"arn:aws:s3:::{s3_bucket}",
                "RoleARN": "arn:aws:iam::000000000000:role/firehose",
            }
        )

        try:
            # Describe
            response = firehose_client.describe_delivery_stream(
                DeliveryStreamName=stream_name
            )
            desc = response["DeliveryStreamDescription"]
            assert desc["DeliveryStreamName"] == stream_name
            assert desc["DeliveryStreamStatus"] == "ACTIVE"

            # List
            response = firehose_client.list_delivery_streams()
            assert stream_name in response["DeliveryStreamNames"]
        finally:
            # Delete
            firehose_client.delete_delivery_stream(DeliveryStreamName=stream_name)

    def test_put_record(self, firehose_client, s3_bucket):
        """Test putting a single record."""
        stream_name = f"test-{uuid.uuid4().hex[:8]}"

        firehose_client.create_delivery_stream(
            DeliveryStreamName=stream_name,
            DeliveryStreamType="DirectPut",
            ExtendedS3DestinationConfiguration={
                "BucketARN": f"arn:aws:s3:::{s3_bucket}",
                "RoleARN": "arn:aws:iam::000000000000:role/firehose",
            }
        )

        try:
            # Put record
            data = json.dumps({"event": "test", "value": 123}).encode()
            response = firehose_client.put_record(
                DeliveryStreamName=stream_name,
                Record={"Data": data}
            )

            assert "RecordId" in response
            assert response["Encrypted"] is False
        finally:
            firehose_client.delete_delivery_stream(DeliveryStreamName=stream_name)

    def test_put_record_batch(self, firehose_client, s3_bucket):
        """Test putting multiple records in a batch."""
        stream_name = f"test-{uuid.uuid4().hex[:8]}"

        firehose_client.create_delivery_stream(
            DeliveryStreamName=stream_name,
            DeliveryStreamType="DirectPut",
            ExtendedS3DestinationConfiguration={
                "BucketARN": f"arn:aws:s3:::{s3_bucket}",
                "RoleARN": "arn:aws:iam::000000000000:role/firehose",
            }
        )

        try:
            # Put batch
            records = [
                {"Data": json.dumps({"id": i}).encode()}
                for i in range(10)
            ]

            response = firehose_client.put_record_batch(
                DeliveryStreamName=stream_name,
                Records=records
            )

            assert response["FailedPutCount"] == 0
            assert len(response["RequestResponses"]) == 10
        finally:
            firehose_client.delete_delivery_stream(DeliveryStreamName=stream_name)


# ============================================
# CloudWatch Logs Tests
# ============================================

class TestCloudWatchLogs:
    def test_log_group_lifecycle(self, logs_client):
        """Test create, describe, and delete log group."""
        group_name = f"/test/{uuid.uuid4().hex[:8]}"

        # Create
        logs_client.create_log_group(logGroupName=group_name)

        try:
            # Describe
            response = logs_client.describe_log_groups(logGroupNamePrefix=group_name)
            groups = [g["logGroupName"] for g in response["logGroups"]]
            assert group_name in groups
        finally:
            # Delete
            logs_client.delete_log_group(logGroupName=group_name)

    def test_log_events(self, logs_client):
        """Test putting and getting log events."""
        group_name = f"/test/{uuid.uuid4().hex[:8]}"
        stream_name = "test-stream"

        logs_client.create_log_group(logGroupName=group_name)
        logs_client.create_log_stream(logGroupName=group_name, logStreamName=stream_name)

        try:
            # Put events
            import time
            now = int(time.time() * 1000)
            logs_client.put_log_events(
                logGroupName=group_name,
                logStreamName=stream_name,
                logEvents=[
                    {"timestamp": now, "message": "Test message 1"},
                    {"timestamp": now + 1, "message": "Test message 2"},
                ]
            )

            # Get events
            response = logs_client.get_log_events(
                logGroupName=group_name,
                logStreamName=stream_name
            )

            messages = [e["message"] for e in response["events"]]
            assert "Test message 1" in messages
            assert "Test message 2" in messages
        finally:
            logs_client.delete_log_group(logGroupName=group_name)
