#!/usr/bin/env python3
"""Generate a deterministic workspace SBOM from Cargo package metadata."""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    raw = subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        text=True,
    )
    metadata = json.loads(raw)
    packages = [
        {
            "name": package["name"],
            "version": package["version"],
            "license": package.get("license"),
            "source": package.get("source") or "workspace",
        }
        for package in metadata["packages"]
    ]
    packages.sort(key=lambda package: (package["name"], package["version"]))
    document = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "components": [
            {
                "type": "library",
                "name": package["name"],
                "version": package["version"],
                "licenses": ([{"license": {"id": package["license"]}}]
                              if package["license"] else []),
                "properties": [{"name": "browsai.source", "value": package["source"]}],
            }
            for package in packages
        ],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"generated SBOM: {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
