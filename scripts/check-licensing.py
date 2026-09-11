#!/usr/bin/env python3
"""Validate the BrowsAI licensing structure.

Checks:

* LICENSE exists
* LICENSE-COMMERCIAL.md exists
* docs/licensing.md exists
* THIRD_PARTY_LICENSES.md exists
* README references both license files
* CONTRIBUTING.md contains the dual-license contributor grant clause
* Cargo.toml does not falsely claim an SPDX identifier
* the LICENSE file preserves the LEGAL REVIEW REQUIRED marker
"""

import re
import sys
from pathlib import Path


def main() -> int:
    repo = Path(__file__).resolve().parent.parent

    errors: list[str] = []
    warnings: list[str] = []

    def must_exist(path: Path, *, name: str) -> None:
        if not path.exists():
            errors.append(f"missing required licensing file: {path.relative_to(repo)} ({name})")
        elif path.stat().st_size == 0:
            errors.append(f"required licensing file is empty: {path.relative_to(repo)}")

    license_text = (repo / "LICENSE").read_text(encoding="utf-8") if (repo / "LICENSE").exists() else ""
    commercial_text = (
        (repo / "LICENSE-COMMERCIAL.md").read_text(encoding="utf-8")
        if (repo / "LICENSE-COMMERCIAL.md").exists()
        else ""
    )
    docs_text = (
        (repo / "docs" / "licensing.md").read_text(encoding="utf-8")
        if (repo / "docs" / "licensing.md").exists()
        else ""
    )
    third_party_text = (
        (repo / "THIRD_PARTY_LICENSES.md").read_text(encoding="utf-8")
        if (repo / "THIRD_PARTY_LICENSES.md").exists()
        else ""
    )
    contributing_text = (
        (repo / "CONTRIBUTING.md").read_text(encoding="utf-8")
        if (repo / "CONTRIBUTING.md").exists()
        else ""
    )
    readme_text = (
        (repo / "README.md").read_text(encoding="utf-8")
        if (repo / "README.md").exists()
        else ""
    )
    workspace_cargo = (repo / "Cargo.toml").read_text(encoding="utf-8") if (repo / "Cargo.toml").exists() else ""

    must_exist(repo / "LICENSE", name="public licence")
    must_exist(repo / "LICENSE-COMMERCIAL.md", name="commercial licensing pointer")
    must_exist(repo / "docs" / "licensing.md", name="licensing guide")
    must_exist(repo / "THIRD_PARTY_LICENSES.md", name="third-party licence inventory")
    must_exist(repo / "CONTRIBUTING.md", name="contributor terms")

    if license_text:
        if "LEGAL REVIEW REQUIRED" not in license_text:
            errors.append("LICENSE must keep the LEGAL REVIEW REQUIRED marker")
        if "Internal Use" not in license_text or "External Commercial Use" not in license_text:
            errors.append("LICENSE must distinguish Internal Use and External Commercial Use")
        if re.search(r"^license\s*=\s*\"(MIT|Apache-2.0|PolyForm|Noncommercial)", license_text, re.M):
            warnings.append("LICENSE references an SPDX-style licence string; verify the surrounding context is explanatory, not a grant")
        if "[COMMERCIAL CONTACT TO BE INSERTED]" in license_text:
            warnings.append("LICENSE still has the commercial-contact placeholder; replace before public release")

    if commercial_text:
        if "[COMMERCIAL CONTACT TO BE INSERTED]" in commercial_text:
            warnings.append("LICENSE-COMMERCIAL.md still has the commercial-contact placeholder; replace before public release")
        if "LICENSE" not in commercial_text:
            warnings.append("LICENSE-COMMERCIAL.md should reference the public LICENSE file")

    if docs_text:
        if "Internal Use" not in docs_text or "External Commercial Use" not in docs_text:
            errors.append("docs/licensing.md must cover both Internal Use and External Commercial Use")
        if "FAQ" not in docs_text:
            errors.append("docs/licensing.md must include an FAQ section")
        if "third-party" not in docs_text.lower():
            errors.append("docs/licensing.md must cover third-party / dependency licensing")

    if third_party_text:
        if "Servo" not in third_party_text:
            errors.append("THIRD_PARTY_LICENSES.md must call out Servo")
        if "MPL" not in third_party_text:
            errors.append("THIRD_PARTY_LICENSES.md must call out MPL 2.0 for Servo / Servo-derived crates")

    if contributing_text:
        if "dual license" not in contributing_text.lower() and "both" not in contributing_text.lower():
            errors.append("CONTRIBUTING.md must state that contributions may be distributed under both the public and commercial licences")
        if "copyright" not in contributing_text.lower():
            errors.append("CONTRIBUTING.md must mention the contributor copyright grant")

    if readme_text:
        if "LICENSE" not in readme_text:
            errors.append("README.md must reference the public LICENSE file")
        if "LICENSE-COMMERCIAL.md" not in readme_text:
            errors.append("README.md must reference LICENSE-COMMERCIAL.md")

    if workspace_cargo:
        spdx_match = re.search(r"^\s*license\s*=\s*\"([^\"]+)\"", workspace_cargo, re.M)
        if spdx_match:
            value = spdx_match.group(1)
            if value in {"MIT", "Apache-2.0", "PolyForm-Noncommercial-1.0.0"}:
                errors.append(
                    f"[workspace.package] license = \"{value}\" falsely claims a standard SPDX identifier; use license-file = \"LICENSE\" instead"
                )
            else:
                warnings.append(
                    f"[workspace.package] license = \"{value}\" — verify this is a legitimate SPDX identifier or remove and use license-file"
                )
        if "license-file" not in workspace_cargo:
            warnings.append("workspace Cargo.toml does not reference a license-file; BrowsAI licence will not appear in cargo metadata")

    per_crate = list((repo / "crates").glob("*/Cargo.toml")) + list((repo / "apps").glob("*/Cargo.toml"))
    for cargo in per_crate:
        text = cargo.read_text(encoding="utf-8")
        local_match = re.search(r"^\s*license\s*=\s*\"([^\"]+)\"", text, re.M)
        if local_match and local_match.group(1) in {"MIT", "Apache-2.0", "PolyForm-Noncommercial-1.0.0"}:
            errors.append(
                f"{cargo.relative_to(repo)} sets license = \"{local_match.group(1)}\"; use license.workspace = true or remove"
            )

    print(f"checked {len(per_crate)} crate Cargo.toml files")
    print(f"errors: {len(errors)}, warnings: {len(warnings)}")
    for error in errors:
        print(f"  ERROR: {error}")
    for warning in warnings:
        print(f"  WARN:  {warning}")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())