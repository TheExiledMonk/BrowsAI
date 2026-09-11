#!/usr/bin/env python3
"""Run deterministic BrowsAI workload measurements and emit validator artifacts."""

from __future__ import annotations

import argparse
import json
import statistics
import subprocess
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKLOADS = ROOT / "benchmarks/workloads.json"


def command_for(workload: dict[str, object]) -> list[str]:
    if "package" in workload:
        return ["cargo", "test", "-q", "-p", str(workload["package"]), "--no-run"]
    return ["cargo", "run", "-q", "-p", "browsai-cli", "--", *workload["command"]]


def measure(workload: dict[str, object], samples: int) -> dict[str, object]:
    elapsed: list[float] = []
    output_bytes = 0
    for _ in range(samples):
        started = time.perf_counter()
        result = subprocess.run(
            command_for(workload), cwd=ROOT, check=True, capture_output=True, text=True
        )
        elapsed.append((time.perf_counter() - started) * 1000.0)
        output_bytes = max(output_bytes, len(result.stdout.encode()) + len(result.stderr.encode()))
    median = statistics.median(elapsed)
    p95 = sorted(elapsed)[max(0, int(len(elapsed) * 0.95) - 1)]
    output_tokens = (output_bytes + 3) // 4
    return {
        "commit": "working-tree",
        "fixture": "deterministic-suite-v1",
        "workload": workload["name"],
        "sample_count": samples,
        "median": median,
        "p95": max(p95, median),
        "snapshot_tokens": output_tokens if "snapshot" in str(workload["name"]) else 0,
        "query_tokens": output_tokens if "query" in str(workload["name"]) else 0,
        "diff_tokens": output_tokens if "diff" in str(workload["name"]) else 0,
        "provenance_tokens": output_tokens if "provenance" in str(workload["name"]) else 0,
        "task_tokens": output_tokens if "task" in str(workload["name"]) else 0,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-dir", type=Path, default=ROOT / "benchmarks/results")
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--check-only", action="store_true")
    args = parser.parse_args()
    manifest = json.loads(WORKLOADS.read_text(encoding="utf-8"))
    workloads = manifest.get("workloads", [])
    if manifest.get("schema_version") != 1 or len(workloads) < 10:
        raise SystemExit("invalid benchmark workload registry")
    if args.check_only:
        print(json.dumps({"valid": True, "workloads": len(workloads)}))
        return 0
    if args.samples < 1:
        raise SystemExit("samples must be positive")
    args.output_dir.mkdir(parents=True, exist_ok=True)
    for workload in workloads:
        result = measure(workload, args.samples)
        output = args.output_dir / f"{workload['name']}.json"
        output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"valid": True, "workloads": len(workloads), "output": str(args.output_dir)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

