"""
In-process integration tests for RustStack.

These tests verify RustStack works directly in Python (no Docker).

Prerequisites:
    - Build the extension: cd ruststack-py && pip install maturin && maturin develop
    - Or: pip install ruststack-py (when published)

Usage:
    pytest tests/integration/test_inprocess.py -v
"""
import pytest
import json

try:
    import ruststack_py
except ImportError:
    pytest.skip("ruststack_py not installed", allow_module_level=True)


class TestS3InProcess:
    """S3 in-process tests."""

    @pytest.fixture
    def rs(self):
        return ruststack_py.RustStack()

    def test_create_bucket(self, rs):
        result = rs.s3_create_bucket("test-bucket")
        assert result == "test-bucket"

    def test_bucket_exists_after_create(self, rs):
        rs.s3_create_bucket("exists-bucket")
        assert rs.s3_bucket_exists("exists-bucket") is True

    def test_bucket_not_exists(self, rs):
        assert rs.s3_bucket_exists("nonexistent-bucket") is False

    def test_list_buckets_empty(self, rs):
        buckets = rs.s3_list_buckets()
        assert buckets == []

    def test_list_buckets_after_create(self, rs):
        rs.s3_create_bucket("bucket1")
        rs.s3_create_bucket("bucket2")
        buckets = rs.s3_list_buckets()
        assert "bucket1" in buckets
        assert "bucket2" in buckets

    def test_put_and_get_object(self, rs):
        rs.s3_create_bucket("my-bucket")
        rs.s3_put_object("my-bucket", "key.txt", "Hello World")
        
        result = rs.s3_get_object("my-bucket", "key.txt")
        assert result is not None
        assert result == "Hello World"

    def test_get_nonexistent_object(self, rs):
        rs.s3_create_bucket("get-bucket")
        result = rs.s3_get_object("get-bucket", "nonexistent.txt")
        assert result is None

    def test_list_objects_empty(self, rs):
        rs.s3_create_bucket("empty-bucket")
        objects = rs.s3_list_objects("empty-bucket")
        assert objects == []

    def test_list_objects_after_put(self, rs):
        rs.s3_create_bucket("list-bucket")
        rs.s3_put_object("list-bucket", "file1.txt", "content1")
        rs.s3_put_object("list-bucket", "file2.txt", "content2")
        
        objects = rs.s3_list_objects("list-bucket")
        assert len(objects) == 2

    def test_delete_object(self, rs):
        rs.s3_create_bucket("delete-bucket")
        rs.s3_put_object("delete-bucket", "to-delete.txt", "content")
        
        rs.s3_delete_object("delete-bucket", "to-delete.txt")
        
        result = rs.s3_get_object("delete-bucket", "to-delete.txt")
        assert result is None

    def test_delete_bucket(self, rs):
        rs.s3_create_bucket("bucket-to-delete")
        rs.s3_delete_bucket("bucket-to-delete")
        
        assert rs.s3_bucket_exists("bucket-to-delete") is False


class TestSQSInProcess:
    """SQS in-process tests."""

    @pytest.fixture
    def rs(self):
        return ruststack_py.RustStack()

    def test_create_queue(self, rs):
        result = rs.sqs_create_queue("test-queue")
        assert "test-queue" in result

    def test_get_queue_url(self, rs):
        rs.sqs_create_queue("my-queue")
        url = rs.sqs_get_queue_url("my-queue")
        assert "my-queue" in url

    def test_send_and_receive_message(self, rs):
        rs.sqs_create_queue("message-queue")
        msg_id = rs.sqs_send_message("message-queue", "hello world")
        assert msg_id is not None
        
        messages = rs.sqs_receive_message("message-queue", 10)
        assert len(messages) == 1
        assert "hello world" in messages[0]

    def test_receive_multiple_messages(self, rs):
        rs.sqs_create_queue("multi-queue")
        rs.sqs_send_message("multi-queue", "msg1")
        rs.sqs_send_message("multi-queue", "msg2")
        rs.sqs_send_message("multi-queue", "msg3")
        
        messages = rs.sqs_receive_message("multi-queue", 10)
        assert len(messages) == 3

    def test_list_queues_empty(self, rs):
        queues = rs.sqs_list_queues()
        assert queues == []

    def test_list_queues_after_create(self, rs):
        rs.sqs_create_queue("queue1")
        rs.sqs_create_queue("queue2")
        
        queues = rs.sqs_list_queues()
        assert any("queue1" in q for q in queues)
        assert any("queue2" in q for q in queues)

    def test_list_queues_with_prefix(self, rs):
        rs.sqs_create_queue("test-queue1")
        rs.sqs_create_queue("test-queue2")
        rs.sqs_create_queue("other-queue")
        
        queues = rs.sqs_list_queues(prefix="test-")
        assert len(queues) == 2

    def test_delete_queue(self, rs):
        rs.sqs_create_queue("queue-to-delete")
        rs.sqs_delete_queue("queue-to-delete")


class TestSecretsManagerInProcess:
    """Secrets Manager in-process tests."""

    @pytest.fixture
    def rs(self):
        return ruststack_py.RustStack()

    def test_create_secret(self, rs):
        result = rs.secrets_create_secret("my-secret", "secret-value")
        assert result == "my-secret"

    def test_get_secret_value(self, rs):
        rs.secrets_create_secret("test-secret", "my-secret-value")
        
        result = rs.secrets_get_secret_value("test-secret")
        assert result == "my-secret-value"

    def test_get_nonexistent_secret(self, rs):
        result = rs.secrets_get_secret_value("nonexistent")
        assert result is None

    def test_update_secret(self, rs):
        rs.secrets_create_secret("update-secret", "v1")
        rs.secrets_put_secret_value("update-secret", "v2")
        
        result = rs.secrets_get_secret_value("update-secret")
        assert result == "v2"

    def test_list_secrets(self, rs):
        rs.secrets_create_secret("secret1", "value1")
        rs.secrets_create_secret("secret2", "value2")
        
        secrets = rs.secrets_list_secrets()
        assert "secret1" in secrets
        assert "secret2" in secrets

    def test_list_secrets_empty(self, rs):
        secrets = rs.secrets_list_secrets()
        assert secrets == []

    def test_describe_secret(self, rs):
        rs.secrets_create_secret("describe-secret", "value")
        
        result = rs.secrets_describe_secret("describe-secret")
        assert result is not None
        data = json.loads(result)
        assert data["name"] == "describe-secret"
        assert "arn" in data

    def test_describe_nonexistent_secret(self, rs):
        result = rs.secrets_describe_secret("nonexistent")
        assert result is None

    def test_delete_secret(self, rs):
        rs.secrets_create_secret("to-delete", "value")
        rs.secrets_delete_secret("to-delete")


class TestFirehoseInProcess:
    """Firehose in-process tests."""

    @pytest.fixture
    def rs(self):
        return ruststack_py.RustStack()

    def test_create_delivery_stream(self, rs):
        result = rs.firehose_create_delivery_stream("test-stream")
        assert result == "test-stream"

    def test_list_delivery_streams_empty(self, rs):
        streams = rs.firehose_list_delivery_streams()
        assert streams == []

    def test_list_delivery_streams_after_create(self, rs):
        rs.firehose_create_delivery_stream("stream1")
        rs.firehose_create_delivery_stream("stream2")
        
        streams = rs.firehose_list_delivery_streams()
        assert "stream1" in streams
        assert "stream2" in streams

    def test_describe_delivery_stream(self, rs):
        rs.firehose_create_delivery_stream("my-stream")
        
        result = rs.firehose_describe_delivery_stream("my-stream")
        assert result is not None
        data = json.loads(result)
        assert data["name"] == "my-stream"

    def test_describe_nonexistent_stream(self, rs):
        result = rs.firehose_describe_delivery_stream("nonexistent")
        assert result is None

    def test_put_record(self, rs):
        rs.firehose_create_delivery_stream("data-stream")
        record_id = rs.firehose_put_record("data-stream", "test data")
        assert record_id is not None

    def test_put_record_batch(self, rs):
        rs.firehose_create_delivery_stream("batch-stream")
        count = rs.firehose_put_record_batch("batch-stream", ["record1", "record2", "record3"])
        assert count == 3

    def test_delete_delivery_stream(self, rs):
        rs.firehose_create_delivery_stream("to-delete")
        rs.firehose_delete_delivery_stream("to-delete")


class TestIAMInProcess:
    """IAM in-process tests."""

    @pytest.fixture
    def rs(self):
        return ruststack_py.RustStack()

    def test_create_role(self, rs):
        policy = '{"Version": "2012-10-17", "Statement": [{"Effect": "Allow", "Action": "s3:*", "Resource": "*"}]}'
        result = rs.iam_create_role("test-role", policy)
        assert result == "test-role"

    def test_get_role(self, rs):
        policy = '{"Version": "2012-10-17", "Statement": []}'
        rs.iam_create_role("existing-role", policy)
        
        result = rs.iam_get_role("existing-role")
        assert result is not None
        data = json.loads(result)
        assert data["name"] == "existing-role"

    def test_get_nonexistent_role(self, rs):
        result = rs.iam_get_role("nonexistent")
        assert result is None

    def test_list_roles_empty(self, rs):
        roles = rs.iam_list_roles()
        assert roles == []

    def test_list_roles_after_create(self, rs):
        policy = '{"Version": "2012-10-17", "Statement": []}'
        rs.iam_create_role("role1", policy)
        rs.iam_create_role("role2", policy)
        
        roles = rs.iam_list_roles()
        assert "role1" in roles
        assert "role2" in roles

    def test_delete_role(self, rs):
        policy = '{"Version": "2012-10-17", "Statement": []}'
        rs.iam_create_role("to-delete", policy)
        rs.iam_delete_role("to-delete")


class TestSNSInProcess:
    """SNS in-process tests."""

    @pytest.fixture
    def rs(self):
        return ruststack_py.RustStack()

    def test_create_topic(self, rs):
        result = rs.sns_create_topic("test-topic")
        assert result == "test-topic"

    def test_list_topics_empty(self, rs):
        topics = rs.sns_list_topics()
        assert topics == []

    def test_list_topics_after_create(self, rs):
        rs.sns_create_topic("topic1")
        rs.sns_create_topic("topic2")
        
        topics = rs.sns_list_topics()
        assert "topic1" in topics
        assert "topic2" in topics

    def test_publish(self, rs):
        rs.sns_create_topic("publish-topic")
        message_id = rs.sns_publish("publish-topic", "test message")
        assert message_id is not None

    def test_subscribe(self, rs):
        rs.sns_create_topic("sub-topic")
        arn = rs.sns_subscribe("sub-topic", "sqs", "http://localhost:4566/queue")
        assert arn is not None

    def test_delete_topic(self, rs):
        rs.sns_create_topic("to-delete")
        rs.sns_delete_topic("to-delete")


class TestDynamoDBInProcess:
    """DynamoDB in-process tests - should use Docker."""

    @pytest.fixture
    def rs(self):
        return ruststack_py.RustStack()

    def test_dynamodb_not_implemented(self, rs):
        with pytest.raises(NotImplementedError):
            rs.ddb_create_table("test", "id", "S")


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
