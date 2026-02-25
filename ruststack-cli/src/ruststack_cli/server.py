"""Server process management for RustStack."""

from __future__ import annotations

import contextlib
import logging
import os
import signal
import socket
import subprocess
import time
from pathlib import Path
from typing import Generator, Optional

import requests

from ruststack_cli import downloader

logger = logging.getLogger(__name__)

DEFAULT_HOST = "127.0.0.1"
DEFAULT_PORT = 4566
DEFAULT_ENDPOINT = f"http://{DEFAULT_HOST}:{DEFAULT_PORT}"
READY_TIMEOUT = 30
READY_CHECK_INTERVAL = 0.1


def find_free_port(start_port: int = 4566, max_attempts: int = 100) -> int:
    """Find a free port starting from start_port."""
    for port in range(start_port, start_port + max_attempts):
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
            try:
                sock.bind(("127.0.0.1", port))
                return port
            except OSError:
                continue
    raise RuntimeError("Could not find a free port")


def wait_for_endpoint(
    endpoint: str,
    timeout: float = READY_TIMEOUT,
    interval: float = READY_CHECK_INTERVAL,
) -> bool:
    """Wait for the endpoint to become ready."""
    health_url = f"{endpoint}/health"
    start_time = time.time()

    while time.time() - start_time < timeout:
        try:
            response = requests.get(health_url, timeout=1)
            if response.status_code == 200:
                return True
        except requests.ConnectionError:
            pass
        except requests.Timeout:
            pass

        time.sleep(interval)

    return False


def is_port_in_use(host: str, port: int) -> bool:
    """Check if a port is in use."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        try:
            sock.bind((host, port))
            return False
        except OSError:
            return True


class RustStackProcess:
    """Manages a RustStack server process."""

    def __init__(
        self,
        host: str = DEFAULT_HOST,
        port: int | None = None,
        endpoint: str | None = None,
        binary_path: Path | str | None = None,
        env: dict[str, str] | None = None,
        log_level: str = "info",
    ):
        self.host = host
        self.port = port or (DEFAULT_PORT if not is_port_in_use(host, DEFAULT_PORT) else find_free_port())
        self.endpoint = endpoint or f"http://{self.host}:{self.port}"
        self.binary_path = Path(binary_path) if binary_path else None
        self.env = env or {}
        self.log_level = log_level
        self._process: subprocess.Popen | None = None

    def _get_binary_path(self) -> Path:
        """Get the path to the ruststack binary."""
        if self.binary_path and self.binary_path.exists():
            return self.binary_path

        binary = downloader.ensure_installed()
        if binary is None:
            raise RuntimeError("Failed to install ruststack binary")

        return binary

    def _build_command(self) -> list[str]:
        """Build the command to start ruststack."""
        cmd = [str(self._get_binary_path())]
        cmd.extend(["--host", self.host])
        cmd.extend(["--port", str(self.port)])

        if self.log_level:
            cmd.extend(["--log-level", self.log_level])

        return cmd

    def _build_env(self) -> dict[str, str]:
        """Build the environment for the process."""
        env = os.environ.copy()
        env.update(self.env)
        if self.log_level.upper() != "DEBUG":
            env["RUST_LOG"] = self.log_level.lower()
        return env

    @property
    def is_running(self) -> bool:
        """Check if the process is running."""
        if self._process is None:
            return False
        return self._process.poll() is None

    def start(self, wait: bool = True) -> "RustStackProcess":
        """Start the RustStack server."""
        if self.is_running:
            logger.warning("RustStack is already running")
            return self

        cmd = self._build_command()
        env = self._build_env()

        logger.info(f"Starting RustStack: {' '.join(cmd)}")

        self._process = subprocess.Popen(
            cmd,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        if wait:
            self.wait_until_ready()

        return self

    def stop(self, timeout: float = 10) -> None:
        """Stop the RustStack server."""
        if not self.is_running or self._process is None:
            return

        logger.info("Stopping RustStack")

        try:
            self._process.terminate()
            self._process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            logger.warning("RustStack did not stop gracefully, killing")
            self._process.kill()
            self._process.wait()

        self._process = None

    def restart(self) -> "RustStackProcess":
        """Restart the RustStack server."""
        self.stop()
        return self.start()

    def wait_until_ready(self, timeout: float = READY_TIMEOUT) -> bool:
        """Wait until the server is ready to accept requests."""
        logger.info(f"Waiting for RustStack at {self.endpoint}")
        if wait_for_endpoint(self.endpoint, timeout):
            logger.info(f"RustStack ready at {self.endpoint}")
            return True
        logger.error(f"RustStack failed to start within {timeout}s")
        return False

    def __enter__(self) -> "RustStackProcess":
        """Context manager entry."""
        return self.start()

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        """Context manager exit."""
        self.stop()

    def __del__(self) -> None:
        """Destructor to ensure process is stopped."""
        if self.is_running:
            self.stop()


class RustStackServer:
    """Manages a running RustStack server with additional utilities."""

    def __init__(
        self,
        host: str = DEFAULT_HOST,
        port: int | None = None,
        process: RustStackProcess | None = None,
    ):
        self.host = host
        self.port = port or DEFAULT_PORT
        self.endpoint = f"http://{self.host}:{self.port}"
        self._process = process

    @classmethod
    def start(
        cls,
        host: str = DEFAULT_HOST,
        port: int | None = None,
        wait: bool = True,
        **kwargs,
    ) -> "RustStackServer":
        """Start a new RustStack server."""
        process = RustStackProcess(host=host, port=port, **kwargs)
        process.start(wait=wait)
        return cls(host=host, port=port, process=process)

    @property
    def is_running(self) -> bool:
        """Check if the server is running."""
        if self._process:
            return self._process.is_running
        return is_port_in_use(self.host, self.port)

    def stop(self) -> None:
        """Stop the server."""
        if self._process:
            self._process.stop()

    def reset(self) -> None:
        """Reset the server state (stub - clears in-memory state)."""
        try:
            requests.post(f"{self.endpoint}/_reset", timeout=5)
        except Exception:
            pass

    def __enter__(self) -> "RustStackServer":
        """Context manager entry."""
        if not self.is_running:
            self._process = RustStackProcess(host=self.host, port=self.port)
            self._process.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        """Context manager exit."""
        self.stop()


@contextlib.contextmanager
def run_ruststack(
    host: str = DEFAULT_HOST,
    port: int | None = None,
    **kwargs,
) -> Generator[RustStackServer, None, None]:
    """Context manager to run RustStack for the duration of a block."""
    server = RustStackServer.start(host=host, port=port, wait=True, **kwargs)
    try:
        yield server
    finally:
        server.stop()
