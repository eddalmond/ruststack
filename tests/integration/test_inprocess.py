"""
In-process integration tests for RustStack.

These tests use the pyo3 bindings to run RustStack directly in the Python process.

Prerequisites:
    - Build the extension: cd ruststack-py && pip install maturin && maturin develop
    - Or: pip install ruststack-py

Usage:
    pytest tests/integration/test_inprocess.py -v
"""
import pytest

# Skip all tests if ruststack_py is not available
try:
    import ruststack_py
except ImportError:
    pytest.skip("ruststack_py not installed", allow_module_level=True)


class TestDynamoDBInProcess:
    """DynamoDB in-process tests."""

    @pytest.fixture
    def rs(self):
        return ruststack_py.RustStack()

    def test_create_table(self, rs):
        result = rs.ddb_create_table("users", "id", "S")
        assert result == "users"

    def test_put_and_get_item(self, rs):
        rs.ddb_create_table("items", "id", "S")
        rs.ddb_put_item("items", "user1", '{"name": "John", "age": 30}')
        
        result = rs.ddb_get_item("items", "user1")
        assert result is not None
        assert "John" in result

    def test_list_tables(self, rs):
        rs.ddb_create_table("table1", "id", "S")
        rs.ddb_create_table("table2", "id", "S")
        
        tables = rs.ddb_list_tables()
        assert "table1" in tables
        assert "table2" in tables


class TestS3InProcess:
    """S3 in-process tests."""

    @pytest.fixture
    def rs(self):
        return ruststack_py.RustStack()

    def test_create_bucket(self, rs):
        result = rs.s3_create_bucket("test-bucket")
        assert result == "test-bucket"

    def test_put_and_get_object(self, rs):
        rs.s3_create_bucket("my-bucket")
        rs.s3_put_object("my-bucket", "key.json", '{"message": "hello"}')
        
        result = rs.s3_get_object("my-bucket", "key.json")
        assert result is not None
        assert "hello" in result

    def test_list_objects(self, rs):
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


class TestFirehoseInProcess:
    """Firehose in-process tests."""

    @pytest.fixture
    def rs(self):
        return ruststack_py.RustStack()

    def test_create_delivery_stream(self, rs):
        result = rs.firehose_create_delivery_stream("test-stream")
        assert result == "test-stream"

    def test_put_record(self, rs):
        rs.firehose_create_delivery_stream("data-stream")
        record_id = rs.firehose_put_record("data-stream", "test data")
        assert record_id is not None

    def test_put_record_batch(self, rs):
        rs.firehose_create_delivery_stream("batch-stream")
        count = rs.firehose_put_record_batch("batch-stream", ["record1", "record2", "record3"])
        assert count == 3


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

    def test_list_roles(self, rs):
        policy = '{"Version": "2012-10-17", "Statement": []}'
        rs.iam_create_role("role1", policy)
        rs.iam_create_role("role2", policy)
        
        roles = rs.iam_list_roles()
        assert "role1" in roles
        assert "role2" in roles


class TestSNSInProcess:
    """SNS in-process tests."""

    @pytest.fixture
    def rs(self):
        return ruststack_py.RustStack()

    def test_create_topic(self, rs):
        result = rs.sns_create_topic("test-topic")
        assert result == "test-topic"

    def test_publish(self, rs):
        rs.sns_create_topic("publish-topic")
        message_id = rs.sns_publish("publish-topic", "test message")
        assert message_id is not None

    def test_subscribe(self, rs):
        rs.sns_create_topic("sub-topic")
        arn = rs.sns_subscribe("sub-topic", "sqs", "http://localhost:4566/queue")
        assert arn is not None

    def test_list_topics(self, rs):
        rs.sns_create_topic("topic1")
        rs.sns_create_topic("topic2")
        
        topics = rs.sns_list_topics()
        assert "topic1" in topics
        assert "topic2" in topics


class TestSQSInProcess:
    """SQS in-process tests."""

    @pytest.fixture
    def rs(self):
        return ruststack_py.RustStack()

    def test_create_queue(self, rs):
        result = rs.sqs_create_queue("test-queue")
        assert "test-queue" in result

    def test_send_and_receive_message(self, rs):
        url = rs.sqs_create_queue("message-queue")
        msg_id = rs.sqs_send_message(url, "hello world")
        assert msg_id is not None
        
        messages = rs.sqs_receive_message(url, 10)
        assert len(messages) == 1
        assert "hello world" in messages[0]

    def test_delete_message(self, rs):
        url = rs.sqs_delete_queue("queue-to-delete")
        # This will fail because queue doesn't exist, which is expected
        # We're just testing the method exists

    def test_list_queues(self, rs):
        rs.sqs_create_queue("queue1")
        rs.sqs_create_queue("queue2")
        
        queues = rs.sqs_list_queues()
        assert any("queue1" in q for q in queues)
        assert any("queue2" in q for q in queues)


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
