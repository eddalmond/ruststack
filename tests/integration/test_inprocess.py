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

    def test_delete_message(self, rs):
        rs.sqs_create_queue("delete-queue")
        rs.sqs_send_message("delete-queue", "to-delete")
        
        messages = rs.sqs_receive_message("delete-queue", 1)
        receipt = messages[0]  # Simplified - need proper receipt handle
        
        # Note: Actual delete requires receipt handle from receive
        # This test just verifies basic flow

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
        
        # Queue should be gone (get_queue_url would fail)


class TestDynamoDBInProcess:
    """DynamoDB in-process tests - should use Docker."""

    @pytest.fixture
    def rs(self):
        return ruststack_py.RustStack()

    def test_dynamodb_not_implemented(self, rs):
        with pytest.raises(NotImplementedError):
            rs.ddb_create_table("test", "id", "S")


class TestSecretsManagerInProcess:
    """Secrets Manager in-process tests - should use Docker."""

    @pytest.fixture
    def rs(self):
        return ruststack_py.RustStack()

    def test_secrets_not_implemented(self, rs):
        with pytest.raises(NotImplementedError):
            rs.secrets_create_secret("test", "value")


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
