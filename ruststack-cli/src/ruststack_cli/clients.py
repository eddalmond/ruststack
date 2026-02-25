"""Client helpers for RustStack."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Optional

import boto3
from botocore.config import Config

if TYPE_CHECKING:
    from mypy_boto3 import s3, dynamodb, lambda_, sqs, sns


DEFAULT_AWS_ACCESS_KEY = "test"
DEFAULT_AWS_SECRET_KEY = "test"
DEFAULT_REGION = "us-east-1"


class RustStackClient:
    """Helper class for creating boto3 clients configured for RustStack."""

    def __init__(
        self,
        endpoint: str = "http://localhost:4566",
        aws_access_key_id: str = DEFAULT_AWS_ACCESS_KEY,
        aws_secret_access_key: str = DEFAULT_AWS_SECRET_KEY,
        region_name: str = DEFAULT_REGION,
        verify: bool = False,
    ):
        self.endpoint = endpoint
        self.aws_access_key_id = aws_access_key_id
        self.aws_secret_access_key = aws_secret_access_key
        self.region_name = region_name
        self.verify = verify

    def _create_client(self, service_name: str, **kwargs) -> Any:
        """Create a boto3 client for the given service."""
        config = Config(
            retries={"max_attempts": 0},
            **kwargs,
        )

        return boto3.client(
            service_name,
            endpoint_url=self.endpoint,
            aws_access_key_id=self.aws_access_key_id,
            aws_secret_access_key=self.aws_secret_access_key,
            region_name=self.region_name,
            verify=self.verify,
            config=config,
        )

    def s3(self, **kwargs) -> "s3.Client":
        """Create an S3 client."""
        return self._create_client("s3", **kwargs)

    def dynamodb(self, **kwargs) -> "dynamodb.Client":
        """Create a DynamoDB client."""
        return self._create_client("dynamodb", **kwargs)

    def dynamodb_resource(self, **kwargs) -> "dynamodb.ServiceResource":
        """Create a DynamoDB resource."""
        return boto3.resource(
            "dynamodb",
            endpoint_url=self.endpoint,
            aws_access_key_id=self.aws_access_key_id,
            aws_secret_access_key=self.aws_secret_access_key,
            region_name=self.region_name,
            verify=self.verify,
            **kwargs,
        )

    def lambda_(self, **kwargs) -> "lambda_.Client":
        """Create a Lambda client."""
        return self._create_client("lambda", **kwargs)

    def sqs(self, **kwargs) -> "sqs.Client":
        """Create an SQS client."""
        return self._create_client("sqs", **kwargs)

    def sns(self, **kwargs) -> "sns.Client":
        """Create an SNS client."""
        return self._create_client("sns", **kwargs)

    def secretsmanager(self, **kwargs) -> Any:
        """Create a Secrets Manager client."""
        return self._create_client("secretsmanager", **kwargs)

    def apigatewayv2(self, **kwargs) -> Any:
        """Create an API Gateway V2 client."""
        return self._create_client("apigatewayv2", **kwargs)

    def logs(self, **kwargs) -> Any:
        """Create a CloudWatch Logs client."""
        return self._create_client("logs", **kwargs)

    def iam(self, **kwargs) -> Any:
        """Create an IAM client."""
        return self._create_client("iam", **kwargs)

    def firehose(self, **kwargs) -> Any:
        """Create a Kinesis Firehose client."""
        return self._create_client("firehose", **kwargs)


def s3_client(
    endpoint: str = "http://localhost:4566",
    aws_access_key_id: str = DEFAULT_AWS_ACCESS_KEY,
    aws_secret_access_key: str = DEFAULT_AWS_SECRET_KEY,
    region_name: str = DEFAULT_REGION,
    **kwargs,
) -> "s3.Client":
    """Create an S3 client configured for RustStack."""
    return RustStackClient(
        endpoint=endpoint,
        aws_access_key_id=aws_access_key_id,
        aws_secret_access_key=aws_secret_access_key,
        region_name=region_name,
    ).s3(**kwargs)


def dynamodb_client(
    endpoint: str = "http://localhost:4566",
    aws_access_key_id: str = DEFAULT_AWS_ACCESS_KEY,
    aws_secret_access_key: str = DEFAULT_AWS_SECRET_KEY,
    region_name: str = DEFAULT_REGION,
    **kwargs,
) -> "dynamodb.Client":
    """Create a DynamoDB client configured for RustStack."""
    return RustStackClient(
        endpoint=endpoint,
        aws_access_key_id=aws_access_key_id,
        aws_secret_access_key=aws_secret_access_key,
        region_name=region_name,
    ).dynamodb(**kwargs)


def lambda_client(
    endpoint: str = "http://localhost:4566",
    aws_access_key_id: str = DEFAULT_AWS_ACCESS_KEY,
    aws_secret_access_key: str = DEFAULT_AWS_SECRET_KEY,
    region_name: str = DEFAULT_REGION,
    **kwargs,
) -> "lambda_.Client":
    """Create a Lambda client configured for RustStack."""
    return RustStackClient(
        endpoint=endpoint,
        aws_access_key_id=aws_access_key_id,
        aws_secret_access_key=aws_secret_access_key,
        region_name=region_name,
    ).lambda_(**kwargs)


def sqs_client(
    endpoint: str = "http://localhost:4566",
    aws_access_key_id: str = DEFAULT_AWS_ACCESS_KEY,
    aws_secret_access_key: str = DEFAULT_AWS_SECRET_KEY,
    region_name: str = DEFAULT_REGION,
    **kwargs,
) -> "sqs.Client":
    """Create an SQS client configured for RustStack."""
    return RustStackClient(
        endpoint=endpoint,
        aws_access_key_id=aws_access_key_id,
        aws_secret_access_key=aws_secret_access_key,
        region_name=region_name,
    ).sqs(**kwargs)


def sns_client(
    endpoint: str = "http://localhost:4566",
    aws_access_key_id: str = DEFAULT_AWS_ACCESS_KEY,
    aws_secret_access_key: str = DEFAULT_AWS_SECRET_KEY,
    region_name: str = DEFAULT_REGION,
    **kwargs,
) -> "sns.Client":
    """Create an SNS client configured for RustStack."""
    return RustStackClient(
        endpoint=endpoint,
        aws_access_key_id=aws_access_key_id,
        aws_secret_access_key=aws_secret_access_key,
        region_name=region_name,
    ).sns(**kwargs)
