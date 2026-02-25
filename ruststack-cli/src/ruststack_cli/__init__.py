"""RustStack CLI - CLI wrapper for RustStack AWS local emulator."""

__version__ = "0.1.2"

from ruststack_cli.server import RustStackProcess, RustStackServer
from ruststack_cli.clients import RustStackClient
from ruststack_cli.fixtures import (
    ruststack_process,
    ruststack_server,
    s3_client,
    dynamodb_client,
    lambda_client,
    sqs_client,
    sns_client,
)

__all__ = [
    "__version__",
    "RustStackProcess",
    "RustStackServer",
    "RustStackClient",
    "ruststack_process",
    "ruststack_server",
    "s3_client",
    "dynamodb_client",
    "lambda_client",
    "sqs_client",
    "sns_client",
]
