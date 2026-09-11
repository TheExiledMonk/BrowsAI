#!/usr/bin/env python3
"""Validate reproducible BrowsAI benchmark result artifacts."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


REQUIRED_RESULT_FIELDS = {
    "commit",
    "fixture",
    "workload",
    "sample_count",
    "median",
    "p95",
    "snapshot_tokens",
    "query_tokens",
    "diff_tokens",
    "provenance_tokens",
    "task_tokens",
}


def validate_result(result: Any, schema: dict[str, Any] | None = None) -> list[str]:
    """Return deterministic validation errors; an empty list means valid."""
    if not isinstance(result, dict):
        return ["result must be a JSON object"]
    required = REQUIRED_RESULT_FIELDS | set((schema or {}).get("required", ()))
    errors = [f"missing required field: {field}" for field in sorted(required - result.keys())]
    for field in ("commit", "fixture", "workload"):
        if field in result and (not isinstance(result[field], str) or not result[field].strip()):
            errors.append(f"{field} must be a non-empty string")
    for field in sorted(required & {"sample_count", "snapshot_tokens", "query_tokens", "diff_tokens", "provenance_tokens", "task_tokens"}):
        value = result.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            errors.append(f"{field} must be a non-negative integer")
    for field in ("median", "p95"):
        value = result.get(field)
        if not isinstance(value, (int, float)) or isinstance(value, bool) or value < 0:
            errors.append(f"{field} must be a non-negative number")
    if isinstance(result.get("sample_count"), int) and result["sample_count"] == 0:
        errors.append("sample_count must be greater than zero")
    if all(isinstance(result.get(field), (int, float)) for field in ("median", "p95")):
        if result["p95"] < result["median"]:
            errors.append("p95 must be greater than or equal to median")
    return errors


def validate_file(path: Path, schema_path: Path) -> list[str]:
    try:
        result = json.loads(path.read_text(encoding="utf-8"))
        schema = json.loads(schema_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [str(error)]
    return validate_result(result, schema)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("result", type=Path)
    parser.add_argument("--schema", type=Path, default=Path(__file__).parent / "tokens/schema.json")
    args = parser.parse_args()
    errors = validate_file(args.result, args.schema)
    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1
    print(f"valid benchmark result: {args.result}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
