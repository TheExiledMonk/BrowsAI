#!/usr/bin/env python3
"""Validate packaging metadata against the checked-in workspace layout."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    manifest_path = ROOT / "packaging" / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest.get("schemaVersion") != 1:
        raise SystemExit("unsupported packaging manifest schema")
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    version_match = re.search(r'^version\s*=\s*"([^"]+)"', cargo, re.MULTILINE)
    if not version_match or version_match.group(1) != manifest.get("projectVersion"):
        raise SystemExit("packaging version does not match workspace version")
    variants = manifest.get("variants", [])
    if {variant.get("name") for variant in variants} != {"headless", "desktop-shell"}:
        raise SystemExit("packaging variants are incomplete")
    build_script = ROOT / "packaging" / "build.sh"
    if not build_script.is_file() or not build_script.stat().st_mode & 0o111:
        raise SystemExit("packaging build script is missing or not executable")
    for variant in variants:
        package = variant.get("cargoPackage")
        if not package or not (ROOT / "apps" / package.removeprefix("browsai-") / "Cargo.toml").exists():
            # Workspace packages may live in crates rather than apps.
            locations = [ROOT / "apps" / package, ROOT / "apps" / package.removeprefix("browsai-"), ROOT / "crates" / package.removeprefix("browsai-")]
            if not any((location / "Cargo.toml").exists() for location in locations):
                raise SystemExit(f"cargo package is missing: {package}")
        for asset in variant.get("runtimeAssets", []):
            if not (ROOT / asset).exists():
                raise SystemExit(f"runtime asset is missing: {asset}")
    targets = manifest.get("targets", {})
    if set(targets) != {"linux", "macos", "windows"}:
        raise SystemExit("packaging target roadmap is incomplete")
    print(f"valid packaging manifest: {manifest_path.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
