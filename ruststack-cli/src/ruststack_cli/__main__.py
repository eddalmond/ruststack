"""CLI entry point for ruststack."""

import sys

from ruststack_cli.cli import app


def main() -> None:
    """Main entry point."""
    app(prog_name="ruststack")


if __name__ == "__main__":
    main()
