#!/usr/bin/env python3
"""Audit TODO.md structure and unresolved work without hiding incomplete items."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


SECTION_RE = re.compile(r"^## (\d+)\. (.+)$")
CHECKBOX_RE = re.compile(r"^- \[([ x~])\] (.+)$")
EVIDENCE_RE = re.compile(r"^  - (Implementation|Tests|Evidence|Documentation):", re.MULTILINE)


def audit(path: Path) -> tuple[list[str], list[str], list[str], int]:
    lines = path.read_text(encoding="utf-8").splitlines()
    errors: list[str] = []
    unresolved: list[str] = []
    in_progress: list[str] = []
    sections: list[tuple[int, str, int]] = []
    checkbox_count = 0

    for line_number, line in enumerate(lines, start=1):
        section = SECTION_RE.match(line)
        if section:
            sections.append((int(section.group(1)), section.group(2), line_number - 1))
        checkbox = CHECKBOX_RE.match(line)
        if checkbox:
            checkbox_count += 1
            if checkbox.group(1) == " ":
                unresolved.append(f"{line_number}: {checkbox.group(2)}")
            elif checkbox.group(1) == "~":
                in_progress.append(f"{line_number}: {checkbox.group(2)}")

    expected_sections = list(range(1, 81))
    actual_sections = [number for number, _, _ in sections]
    if actual_sections != expected_sections:
        errors.append(
            "section sequence must contain exactly 1..80; "
            f"found {actual_sections!r}"
        )
    if checkbox_count == 0:
        errors.append("no TODO checkboxes found")
    for index, (number, title, start) in enumerate(sections):
        end = sections[index + 1][2] if index + 1 < len(sections) else len(lines)
        block = "\n".join(lines[start:end])
        if not EVIDENCE_RE.search(block):
            errors.append(f"section {number} ({title}) has no evidence annotation")
    return errors, unresolved, in_progress, checkbox_count


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("path", nargs="?", type=Path, default=Path("TODO.md"))
    parser.add_argument(
        "--require-complete",
        action="store_true",
        help="fail when any unchecked item remains",
    )
    args = parser.parse_args()

    errors, unresolved, in_progress, checkbox_count = audit(args.path)
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1

    print(
        f"TODO audit: sections=80 checkboxes={checkbox_count} "
        f"unchecked={len(unresolved)} in_progress={len(in_progress)}"
    )
    for item in unresolved:
        print(f"UNRESOLVED {item}")
    if args.require_complete and (unresolved or in_progress):
        for item in in_progress:
            print(f"IN_PROGRESS {item}")
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
