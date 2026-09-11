#!/usr/bin/env python3
"""Validate that controlled test sites are local, parseable, and complete."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path


REQUIRED_PATHS = (
    "html",
    "css",
    "javascript",
    "forms",
    "frameworks",
    "iframe",
    "shadow-dom",
    "websocket",
    "workers",
    "service-worker",
    "canvas",
    "webgl",
    "media",
    "accessibility",
    "conformance",
    "semantic-golden",
)
URL_RE = re.compile(r"(?:https?|wss?)://([^/\"'\s)]+)", re.IGNORECASE)


def validate(root: Path) -> list[str]:
    errors: list[str] = []
    for relative in REQUIRED_PATHS:
        path = root / relative
        if not path.is_dir() or not any(path.iterdir()):
            errors.append(f"missing fixture group or files: {relative}")

    for path in sorted(root.rglob("*")):
        if not path.is_file() or path.name == "README.md":
            continue
        try:
            content = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for host in URL_RE.findall(content):
            hostname = host.split(":", 1)[0].lower()
            if not (hostname.endswith(".test") or hostname in {"localhost", "127.0.0.1"}):
                errors.append(f"remote dependency in fixture: {path} ({hostname})")
        if path.suffix == ".json":
            try:
                json.loads(content)
            except json.JSONDecodeError as error:
                errors.append(f"invalid JSON fixture {path}: {error}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("root", nargs="?", type=Path, default=Path("test-sites"))
    args = parser.parse_args()
    errors = validate(args.root)
    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1
    print(f"valid controlled test sites: {args.root}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
