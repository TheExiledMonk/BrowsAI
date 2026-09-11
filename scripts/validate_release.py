#!/usr/bin/env python3
"""Validate release policy references and mandatory repository gates."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    policy = json.loads((ROOT / "release/policy.json").read_text(encoding="utf-8"))
    if policy.get("schema_version") != 1 or policy.get("signing_required") is not True:
        raise SystemExit("invalid release policy schema or signing requirement")
    for relative in [
        "Cargo.lock",
        "CHANGELOG.md",
        "docs/release.md",
        "docs/operations.md",
        "packaging/manifest.json",
        policy["rollback_document"],
        policy["migration_document"],
        policy["changelog"],
    ]:
        if not (ROOT / relative).is_file():
            raise SystemExit(f"release input is missing: {relative}")
    manifest = json.loads((ROOT / "packaging/manifest.json").read_text(encoding="utf-8"))
    variants = {variant["name"] for variant in manifest["variants"]}
    if set(policy["artifact_variants"]) != variants:
        raise SystemExit("release artifact variants do not match packaging manifest")
    ci = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
    missing = [gate for gate in policy["required_ci_gates"] if gate not in ci]
    if missing:
        raise SystemExit(f"CI is missing release gates: {', '.join(missing)}")
    if not re.search(r"0\.1\.0|Unreleased", (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")):
        raise SystemExit("changelog has no current or unreleased entry")
    print("valid release policy: release/policy.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

