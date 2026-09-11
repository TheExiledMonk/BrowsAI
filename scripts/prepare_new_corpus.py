#!/usr/bin/env python3
"""Prepare a fresh ranked corpus with no domains from an earlier campaign."""

from __future__ import annotations

import argparse
import csv
import json
import zipfile
from pathlib import Path


def domains_from_csv(path: Path) -> set[str]:
    with path.open(newline="") as handle:
        return {row["domain"].lower().removeprefix("www.") for row in csv.DictReader(handle)}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ranked-zip", required=True)
    parser.add_argument("--old-corpus", action="append", required=True)
    parser.add_argument("--output-corpus", required=True)
    parser.add_argument("--output-state", required=True)
    parser.add_argument("--target", type=int, default=2000)
    args = parser.parse_args()

    excluded = set()
    for path in args.old_corpus:
        excluded |= domains_from_csv(Path(path))

    selected = []
    seen = set()
    with zipfile.ZipFile(args.ranked_zip) as archive:
        with archive.open(archive.namelist()[0]) as handle:
            for raw in handle:
                rank_text, raw_domain = raw.decode("utf-8", "ignore").strip().split(",", 1)
                domain = raw_domain.lower().strip().removeprefix("www.")
                if not domain or "." not in domain or domain in excluded or domain in seen:
                    continue
                seen.add(domain)
                selected.append({
                    "id": f"new-{len(selected) + 1:04d}",
                    "url": f"https://{domain}/",
                    "domain": domain,
                    "category": "ranked-new-corpus",
                    "priority": max(1, 1_000_000 - int(rank_text)),
                    "enabled": "true",
                    "source_rank": int(rank_text),
                })
                if len(selected) >= args.target:
                    break

    if len(selected) < args.target:
        raise SystemExit(f"only found {len(selected)} new domains, wanted {args.target}")

    output = Path(args.output_corpus)
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=["id", "url", "domain", "category", "priority", "enabled"])
        writer.writeheader()
        writer.writerows({key: row[key] for key in writer.fieldnames} for row in selected)

    state = {
        "campaign": "new-ranked-2000",
        "source": str(args.ranked_zip),
        "excluded_corpora": args.old_corpus,
        "sites": {},
    }
    Path(args.output_state).write_text(json.dumps(state, indent=2, sort_keys=True) + "\n")
    print(f"selected={len(selected)} excluded={len(excluded)} first_rank={selected[0]['source_rank']} last_rank={selected[-1]['source_rank']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
