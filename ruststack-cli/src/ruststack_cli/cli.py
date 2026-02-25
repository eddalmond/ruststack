"""CLI commands for ruststack."""

from __future__ import annotations

import logging
import sys
from pathlib import Path

import click

from ruststack_cli import __version__
from ruststack_cli import downloader
from ruststack_cli.server import RustStackProcess, is_port_in_use, wait_for_endpoint

logging.basicConfig(
    level=logging.INFO,
    format="%(levelname)s: %(message)s",
)
logger = logging.getLogger(__name__)


@click.group()
@click.version_option(version=__version__)
def app() -> None:
    """RustStack - High-fidelity AWS local emulator."""
    pass


@app.command()
@click.option(
    "--version",
    "-v",
    help="Specific version to install",
)
@click.option(
    "--force",
    "-f",
    is_flag=True,
    help="Force reinstall even if cached",
)
def install(version: str | None, force: bool) -> None:
    """Install or update RustStack binary."""
    logger.info(f"Installing RustStack (version: {version or 'latest'})")

    binary = downloader.install(version=version, force=force)

    if binary:
        installed_version = downloader.get_installed_version()
        click.echo(f"Successfully installed ruststack {installed_version}")
    else:
        click.echo("Installation failed", err=True)
        sys.exit(1)


@app.command()
@click.option(
    "--host",
    default="127.0.0.1",
    help="Host to bind to",
)
@click.option(
    "--port",
    "-p",
    type=int,
    default=None,
    help="Port to listen on (default: 4566 or next free)",
)
@click.option(
    "--log-level",
    default="info",
    type=click.Choice(["debug", "info", "warn", "error"]),
    help="Log level",
)
@click.option(
    "--wait/--no-wait",
    default=True,
    help="Wait for server to be ready",
)
@click.option(
    "--background",
    "-b",
    is_flag=True,
    help="Run in background (daemon mode)",
)
def start(
    host: str,
    port: int | None,
    log_level: str,
    wait: bool,
    background: bool,
) -> None:
    """Start the RustStack server."""
    if port is None:
        port = 4566
        while is_port_in_use(host, port):
            port += 1

    binary = downloader.ensure_installed()
    if not binary:
        click.echo("RustStack not installed. Run 'ruststack install' first.", err=True)
        sys.exit(1)

    process = RustStackProcess(
        host=host,
        port=port,
        log_level=log_level,
    )

    if background:
        process.start(wait=False)
        click.echo(f"RustStack started at {process.endpoint}")
    else:
        try:
            process.start(wait=wait)
            click.echo(f"RustStack running at {process.endpoint}")

            import time

            while process.is_running:
                time.sleep(1)
        except KeyboardInterrupt:
            click.echo("\nStopping RustStack...")
            process.stop()


@app.command()
@click.option(
    "--host",
    default="127.0.0.1",
    help="Host where RustStack is running",
)
@click.option(
    "--port",
    "-p",
    type=int,
    default=4566,
    help="Port where RustStack is running",
)
@click.option(
    "--timeout",
    "-t",
    type=float,
    default=10,
    help="Timeout in seconds",
)
def wait_ready(host: str, port: int, timeout: float) -> None:
    """Wait for RustStack to be ready."""
    endpoint = f"http://{host}:{port}"
    click.echo(f"Waiting for RustStack at {endpoint}...")

    if wait_for_endpoint(endpoint, timeout=timeout):
        click.echo("RustStack is ready!")
    else:
        click.echo("RustStack failed to start", err=True)
        sys.exit(1)


@app.command()
@click.option(
    "--host",
    default="127.0.0.1",
    help="Host where RustStack is running",
)
@click.option(
    "--port",
    "-p",
    type=int,
    default=4566,
    help="Port where RustStack is running",
)
def status(host: str, port: int) -> None:
    """Check if RustStack is running."""
    endpoint = f"http://{host}:{port}"

    if is_port_in_use(host, port):
        if wait_for_endpoint(endpoint, timeout=2):
            click.echo(f"RustStack is running at {endpoint}")
            sys.exit(0)
        else:
            click.echo(f"Port {port} is in use but RustStack is not responding")
            sys.exit(1)
    else:
        click.echo(f"RustStack is not running (port {port} is free)")
        sys.exit(1)


@app.command()
def stop() -> None:
    """Stop the RustStack server (currently just reports status)."""
    click.echo("To stop RustStack, use 'kill $(pgrep ruststack)' or stop the process manually")
    click.echo("Alternatively, use the process you started in the foreground")


@app.command()
def update() -> None:
    """Update RustStack to the latest version."""
    current = downloader.get_installed_version()
    latest = downloader.get_latest_version()

    if current is None:
        click.echo("RustStack is not installed. Run 'ruststack install' first.")
        sys.exit(1)

    if latest is None:
        click.echo("Could not check for updates. Check your internet connection.")
        sys.exit(1)

    if current == latest:
        click.echo(f"RustStack is already up to date ({current})")
        return

    click.echo(f"Updating from {current} to {latest}...")
    binary = downloader.install(version=latest, force=True)

    if binary:
        click.echo(f"Successfully updated to {latest}")
    else:
        click.echo("Update failed", err=True)
        sys.exit(1)


@app.command()
def version() -> None:
    """Show installed RustStack version."""
    version = downloader.get_installed_version()
    if version:
        click.echo(f"ruststack {version}")
    else:
        click.echo("ruststack is not installed")


@app.command()
def check_updates() -> None:
    """Check if updates are available."""
    result = downloader.check_for_updates()

    if result is None:
        click.echo("RustStack is not installed")
        sys.exit(1)

    current, latest = result

    if current == latest:
        click.echo(f"RustStack is up to date ({current})")
    else:
        click.echo(f"Update available: {current} -> {latest}")
        click.echo("Run 'ruststack update' to install")


if __name__ == "__main__":
    app()
