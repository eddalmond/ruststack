"""Binary downloader for RustStack."""

from __future__ import annotations

import hashlib
import json
import logging
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path
from typing import NamedTuple

import requests
import semver

logger = logging.getLogger(__name__)

REPO_OWNER = "eddalmond"
REPO_NAME = "ruststack"
GITHUB_API_URL = f"https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}"
GITHUB_DOWNLOAD_URL = f"https://github.com/{REPO_OWNER}/{REPO_NAME}/releases/download"


class Platform(NamedTuple):
    """Platform information for binary selection."""

    system: str
    machine: str
    extension: str

    @property
    def artifact_name(self) -> str:
        """Get the artifact name for this platform."""
        system_map = {"Linux": "linux", "Darwin": "macos"}
        system = system_map.get(self.system, self.system.lower())
        if system == "macos" and self.machine == "arm64":
            return f"ruststack-macos-arm64"
        elif system == "macos":
            return f"ruststack-macos-x86_64"
        return f"ruststack-linux-x86_64"


def get_platform() -> Platform:
    """Detect the current platform."""
    system = platform.system()
    machine = platform.machine()

    if system == "Windows":
        ext = ".exe"
    else:
        ext = ""

    return Platform(system=system, machine=machine, extension=ext)


def get_cache_dir() -> Path:
    """Get the cache directory for RustStack binaries."""
    try:
        from platformdirs import user_cache_dir
    except ImportError:
        user_cache = os.path.expanduser("~/.cache")
        return Path(user_cache) / "ruststack"

    return Path(user_cache_dir("ruststack", appauthor=False))


def get_install_dir() -> Path:
    """Get the installation directory for RustStack binaries."""
    try:
        from platformdirs import user_local_bin_dir
    except ImportError:
        if sys.platform == "darwin":
            return Path.home() / ".local" / "bin"
        return Path.home() / ".local" / "bin"

    return Path(user_local_bin_dir())


def get_latest_version() -> str | None:
    """Get the latest version from GitHub releases."""
    try:
        response = requests.get(f"{GITHUB_API_URL}/releases/latest", timeout=10)
        response.raise_for_status()
        data = response.json()
        tag = data.get("tag_name", "")
        if tag.startswith("v"):
            tag = tag[1:]
        return tag
    except Exception as e:
        logger.warning(f"Failed to check latest version: {e}")
        return None


def get_version_assets(version: str, platform: Platform) -> dict | None:
    """Get the download URL and checksum for a specific version."""
    try:
        if version.startswith("v"):
            version = version[1:]

        tag = f"v{version}"
        response = requests.get(f"{GITHUB_API_URL}/releases/tags/{tag}", timeout=10)
        response.raise_for_status()
        data = response.json()

        artifact_base = platform.artifact_name

        download_url = None
        checksum = None

        for asset in data.get("assets", []):
            name = asset["name"]
            if name == f"{artifact_base}.tar.gz":
                download_url = asset["browser_download_url"]
            elif name.endswith(".sha256"):
                checksum_response = requests.get(asset["browser_download_url"], timeout=10)
                checksum = checksum_response.text.split()[0]

        if download_url:
            return {"url": download_url, "checksum": checksum}

        return None
    except Exception as e:
        logger.warning(f"Failed to get release assets: {e}")
        return None


def download_binary(url: str, dest: Path, expected_checksum: str | None = None) -> bool:
    """Download the binary from the given URL."""
    try:
        logger.info(f"Downloading RustStack from {url}")
        response = requests.get(url, timeout=60, stream=True)
        response.raise_for_status()

        dest.parent.mkdir(parents=True, exist_ok=True)

        with open(dest, "wb") as f:
            for chunk in response.iter_content(chunk_size=8192):
                f.write(chunk)

        if expected_checksum:
            actual_checksum = hashlib.sha256(dest.read_bytes()).hexdigest()
            if actual_checksum != expected_checksum:
                logger.error(f"Checksum mismatch: expected {expected_checksum}, got {actual_checksum}")
                dest.unlink()
                return False
            logger.info("Checksum verified")

        dest.chmod(0o755)
        return True
    except Exception as e:
        logger.error(f"Failed to download binary: {e}")
        if dest.exists():
            dest.unlink()
        return False


def find_existing_binary(version: str) -> Path | None:
    """Find an existing binary in the cache."""
    cache_dir = get_cache_dir()
    binary_name = f"ruststack-{version}{get_platform().extension}"
    binary_path = cache_dir / binary_name

    if binary_path.exists():
        logger.info(f"Found cached binary: {binary_path}")
        return binary_path

    return None


def link_or_copy_binary(source: Path, version: str) -> Path:
    """Link or copy the binary to the install directory."""
    install_dir = get_install_dir()
    install_dir.mkdir(parents=True, exist_ok=True)

    binary_name = f"ruststack{get_platform().extension}"
    dest = install_dir / binary_name

    if dest.exists():
        dest.unlink()

    try:
        os.symlink(source, dest)
    except OSError:
        shutil.copy2(source, dest)

    logger.info(f"Installed ruststack to {dest}")
    return dest


def build_from_source() -> bool | Path:
    """Build RustStack from source using cargo."""
    logger.info("Attempting to build from source...")

    cargo_path = shutil.which("cargo")
    if not cargo_path:
        logger.error("cargo not found, cannot build from source")
        return False

    project_root = Path(__file__).parent.parent.parent
    ruststack_dir = project_root / "ruststack"

    if not ruststack_dir.exists():
        logger.error("ruststack source not found")
        return False

    try:
        result = subprocess.run(
            ["cargo", "build", "--release"],
            cwd=ruststack_dir,
            capture_output=True,
            text=True,
        )

        if result.returncode != 0:
            logger.error(f"Build failed: {result.stderr}")
            return False

        binary_path = ruststack_dir / "target" / "release" / "ruststack"
        if not binary_path.exists():
            logger.error("Build completed but binary not found")
            return False

        return binary_path

    except Exception as e:
        logger.error(f"Build failed: {e}")
        return False


def install(version: str | None = None, force: bool = False) -> Path | None:
    """Install RustStack binary."""
    if version is None:
        version = get_latest_version()
        if version is None:
            logger.error("Could not determine latest version")
            return None

    if not force:
        existing = find_existing_binary(version)
        if existing:
            return link_or_copy_binary(existing, version)

    platform_info = get_platform()
    assets = get_version_assets(version, platform_info)

    if assets:
        cache_dir = get_cache_dir()
        binary_name = f"ruststack-{version}{platform_info.extension}"
        cache_path = cache_dir / binary_name

        if download_binary(assets["url"], cache_path, assets.get("checksum")):
            return link_or_copy_binary(cache_path, version)

    logger.warning("Download failed, attempting to build from source")
    source_binary: Path | bool = build_from_source()
    if source_binary and isinstance(source_binary, Path):
        return link_or_copy_binary(source_binary, version)

    logger.error("Installation failed")
    return None


def ensure_installed(version: str | None = None) -> Path | None:
    """Ensure RustStack is installed, installing if necessary."""
    install_dir = get_install_dir()
    binary_name = f"ruststack{get_platform().extension}"
    binary_path = install_dir / binary_name

    if binary_path.exists():
        return binary_path

    return install(version=version)


def get_installed_version() -> str | None:
    """Get the currently installed version."""
    install_dir = get_install_dir()
    binary_name = f"ruststack{get_platform().extension}"
    binary_path = install_dir / binary_name

    if not binary_path.exists():
        return None

    try:
        result = subprocess.run(
            [str(binary_path), "--version"],
            capture_output=True,
            text=True,
            timeout=5,
        )
        if result.returncode == 0:
            output = result.stdout.strip()
            if output.startswith("ruststack "):
                return output.split()[1]
    except Exception:
        pass

    return None


def check_for_updates() -> tuple[str, str] | None:
    """Check if an update is available. Returns (current, latest) or None."""
    current = get_installed_version()
    if current is None:
        return None

    latest = get_latest_version()
    if latest is None:
        return None

    if semver.compare(current, latest) < 0:
        return (current, latest)

    return None
