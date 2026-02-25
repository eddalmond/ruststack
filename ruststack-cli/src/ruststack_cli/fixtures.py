"""Pytest fixtures for RustStack."""

from __future__ import annotations

import pytest

from ruststack_cli.clients import (
    RustStackClient,
    dynamodb_client,
    lambda_client,
    s3_client,
    sns_client,
    sqs_client,
)
from ruststack_cli.server import RustStackProcess, RustStackServer


@pytest.fixture(scope="session")
def ruststack_endpoint() -> str:
    """Return the default RustStack endpoint."""
    return "http://localhost:4566"


@pytest.fixture(scope="session")
def ruststack_process(ruststack_endpoint: str) -> Generator[RustStackProcess, None, None]:
    """Start RustStack for the test session.

    This fixture starts RustStack once for all tests and ensures it's
    stopped after the test session completes.
    """
    process = RustStackProcess()
    process.start(wait=True)
    yield process
    process.stop()


@pytest.fixture(scope="session")
def ruststack_server(ruststack_endpoint: str) -> Generator[RustStackServer, None, None]:
    """Start RustStack for the test session.

    This is an alternative to ruststack_process that provides a more
    object-oriented interface.
    """
    server = RustStackServer.start()
    yield server
    server.stop()


@pytest.fixture
def ruststack(ruststack_process: RustStackProcess) -> str:
    """Return the endpoint URL for RustStack.

    This is a convenience fixture that depends on ruststack_process
    and returns just the endpoint URL.
    """
    return ruststack_process.endpoint


@pytest.fixture
def ruststack_client(ruststack: str) -> RustStackClient:
    """Create a RustStack client."""
    return RustStackClient(endpoint=ruststack)


@pytest.fixture
def s3(ruststack: str) -> "s3.Client":
    """Create an S3 client for RustStack."""
    return s3_client(ruststack)


@pytest.fixture
def dynamodb(ruststack: str) -> "dynamodb.Client":
    """Create a DynamoDB client for RustStack."""
    return dynamodb_client(ruststack)


@pytest.fixture
def dynamodb_resource(ruststack: str) -> "dynamodb.ServiceResource":
    """Create a DynamoDB resource for RustStack."""
    return RustStackClient(endpoint=ruststack).dynamodb_resource()


@pytest.fixture
def lambda_(ruststack: str) -> "lambda_.Client":
    """Create a Lambda client for RustStack."""
    return lambda_client(ruststack)


@pytest.fixture
def sqs(ruststack: str) -> "sqs.Client":
    """Create an SQS client for RustStack."""
    return sqs_client(ruststack)


@pytest.fixture
def sns(ruststack: str) -> "sns.Client":
    """Create an SNS client for RustStack."""
    return sns_client(ruststack)


@pytest.fixture(autouse=True)
def reset_ruststack(ruststack_process: RustStackProcess) -> None:
    """Reset RustStack state after each test.

    This fixture runs after each test to ensure clean state.
    It attempts to clear any in-memory state.
    """
    yield
    try:
        import requests

        requests.post(f"{ruststack_process.endpoint}/_reset", timeout=2)
    except Exception:
        pass


def pytest_configure(config) -> None:
    """Configure pytest with custom markers."""
    config.addinivalue_line(
        "markers", "ruststack: mark test as requiring ruststack"
    )
