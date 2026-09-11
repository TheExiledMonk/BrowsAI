#!/usr/bin/env python3
"""Run the deterministic fixture and browser-harness regression suite."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def run(command: list[str]) -> None:
    subprocess.run(command, cwd=ROOT, check=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check-only", action="store_true")
    args = parser.parse_args()

    run([sys.executable, "scripts/test_run_google_corpus.py"])
    run([sys.executable, "scripts/validate_test_sites.py"])
    run([sys.executable, "scripts/validate_packaging.py"])
    if not args.check_only:
        run([
            "cargo",
            "test",
            "-p",
            "browsai-engine-servo",
            "-p",
            "browsai-semantic-compiler",
            "-p",
            "browsai-action-planner",
            "-p",
            "browsai-agent-runtime",
            "--all-features",
        ])
    print(json.dumps({"valid": True, "mode": "check" if args.check_only else "full"}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
