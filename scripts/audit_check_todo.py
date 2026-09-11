#!/usr/bin/env python3
"""Audit the compatibility-check checklist without changing it."""

import re
import sys
from pathlib import Path


def main() -> int:
    path = Path(sys.argv[1] if len(sys.argv) > 1 else "CHECK_TODO.md")
    text = path.read_text(encoding="utf-8")
    sections = [int(value) for value in re.findall(r"^## (\d+)\.", text, re.MULTILINE)]
    errors = []
    if sections != list(range(1, 126)):
        errors.append("section sequence must contain exactly 1..125")
    unchecked = len(re.findall(r"^- \[ \]", text, re.MULTILINE))
    in_progress = len(re.findall(r"^- \[~\]", text, re.MULTILINE))
    blocked = len(re.findall(r"^- \[!\]", text, re.MULTILINE))
    for match in re.finditer(r"^## (\d+)\. ([^\n]+)\n(.*?)(?=^## |\Z)", text, re.MULTILINE | re.DOTALL):
        number, title, body = match.groups()
        completed = re.findall(r"^- \[x\] ([^\n]+)", body, re.MULTILINE)
        if completed and not any("Evidence:" in item for item in completed):
            errors.append(f"completed section {number} ({title}) lacks evidence")
    print(f"sections={len(sections)} unchecked={unchecked} in_progress={in_progress} blocked={blocked}")
    for error in errors:
        print(f"ERROR: {error}")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
