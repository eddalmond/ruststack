"""Tests for ruststack-cli package."""

from unittest.mock import patch

import pytest


class TestRustStackClient:
    """Tests for RustStackClient."""

    def test_default_endpoint(self):
        """Test default endpoint is correct."""
        from ruststack_cli.clients import RustStackClient

        client = RustStackClient()
        assert client.endpoint == "http://localhost:4566"

    def test_custom_endpoint(self):
        """Test custom endpoint."""
        from ruststack_cli.clients import RustStackClient

        client = RustStackClient(endpoint="http://custom:9999")
        assert client.endpoint == "http://custom:9999"

    def test_default_credentials(self):
        """Test default credentials."""
        from ruststack_cli.clients import RustStackClient

        client = RustStackClient()
        assert client.aws_access_key_id == "test"
        assert client.aws_secret_access_key == "test"
        assert client.region_name == "us-east-1"


class TestDownloader:
    """Tests for downloader module."""

    def test_get_platform(self):
        """Test platform detection."""
        from ruststack_cli.downloader import get_platform

        platform = get_platform()
        assert platform.system in ("Linux", "Darwin", "Windows")
        assert platform.machine in ("x86_64", "arm64", "aarch64", "AMD64")

    def test_artifact_name_linux(self):
        """Test artifact name for Linux."""
        from ruststack_cli.downloader import Platform

        platform = Platform(system="Linux", machine="x86_64", extension="")
        assert platform.artifact_name == "ruststack-linux-x86_64"

    def test_artifact_name_macos_x86(self):
        """Test artifact name for macOS x86_64."""
        from ruststack_cli.downloader import Platform

        platform = Platform(system="Darwin", machine="x86_64", extension="")
        assert platform.artifact_name == "ruststack-macos-x86_64"

    def test_artifact_name_macos_arm(self):
        """Test artifact name for macOS arm64."""
        from ruststack_cli.downloader import Platform

        platform = Platform(system="Darwin", machine="arm64", extension="")
        assert platform.artifact_name == "ruststack-macos-arm64"


class TestServer:
    """Tests for server module."""

    def test_default_port(self):
        """Test default port."""
        from ruststack_cli.server import DEFAULT_PORT

        assert DEFAULT_PORT == 4566

    def test_default_endpoint(self):
        """Test default endpoint."""
        from ruststack_cli.server import DEFAULT_ENDPOINT

        assert DEFAULT_ENDPOINT == "http://127.0.0.1:4566"


class TestCLI:
    """Tests for CLI module."""

    def test_cli_import(self):
        """Test CLI can be imported."""
        from ruststack_cli.cli import app

        assert app is not None
