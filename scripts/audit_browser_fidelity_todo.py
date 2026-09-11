#!/usr/bin/env python3
"""Audit the browser-fidelity checklist without changing it."""

import re
import sys
from pathlib import Path


def main() -> int:
    path = Path(sys.argv[1] if len(sys.argv) > 1 else "BROWSER_FIDELITY_TODO.md")
    text = path.read_text(encoding="utf-8")
    errors = []
    sections = re.findall(r"^## (\d+)\. ([^\n]+)\n(.*?)(?=^## |\Z)", text, re.MULTILINE | re.DOTALL)
    section_numbers = [int(num) for num, _title, _body in sections]
    if not sections:
        errors.append("no sections found")
    unchecked = len(re.findall(r"^- \[ \]", text, re.MULTILINE))
    in_progress = len(re.findall(r"^- \[~\]", text, re.MULTILINE))
    blocked = len(re.findall(r"^- \[!\]", text, re.MULTILINE))
    completed = len(re.findall(r"^- \[x\] ", text, re.MULTILINE))
    for number, title, body in sections:
        items = re.findall(r"^- (\[[ x~!]\]) ([^\n]+)", body, re.MULTILINE)
        for status, item in items:
            if status == "[x]" and "Evidence:" not in item:
                errors.append(
                    f"section {number} ({title}) completed item lacks Evidence: {item[:80]}"
                )
    print(
        f"sections={len(sections)} completed={completed} "
        f"unchecked={unchecked} in_progress={in_progress} blocked={blocked}"
    )
    for error in errors:
        print(f"ERROR: {error}")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())