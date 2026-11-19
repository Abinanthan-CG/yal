#!/usr/bin/env python3
"""Mark every workspace member as not-for-crates.io.

These crates are the private parts of one application. None of them is published
to crates.io, and saying so in the manifest is what lets `cargo deny` accept the
path dependencies between them: a crate marked `publish = false` is private, and
cargo-deny's `allow-wildcard-paths` applies to private crates.

Run from the repository root:

    python scripts/set-publish-false.py

It only adds the line when it is missing, so it is safe to run again.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def workspace_members(root: Path) -> list[Path]:
    """The manifests named by the root Cargo.toml's [workspace] members."""
    text = (root / "Cargo.toml").read_text(encoding="utf-8")
    block = re.search(r"^\s*members\s*=\s*\[(.*?)\]", text, re.S | re.M)
    if block is None:
        raise SystemExit("no [workspace] members array in the root Cargo.toml")
    members = re.findall(r'"([^"]+)"', block.group(1))
    manifests = [root / member / "Cargo.toml" for member in members]
    missing = [m for m in manifests if not m.is_file()]
    if missing:
        for m in missing:
            print(f"missing: {m}", file=sys.stderr)
        raise SystemExit("a workspace member has no manifest")
    return manifests


def add_publish_false(manifest: Path) -> bool:
    """Put `publish = false` in the [package] section, if it is not there."""
    text = manifest.read_text(encoding="utf-8")
    if re.search(r"^publish\s*=", text, re.M):
        return False
    lines = text.split("\n")
    # Find the end of the [package] section.
    start = next((i for i, l in enumerate(lines) if l.strip() == "[package]"), None)
    if start is None:
        raise SystemExit(f"{manifest} has no [package] section")
    end = start + 1
    while end < len(lines) and not lines[end].lstrip().startswith("["):
        end += 1
    # Insert after the last non-empty line of the section.
    insert_at = end
    while insert_at - 1 > start and lines[insert_at - 1].strip() == "":
        insert_at -= 1
    lines.insert(insert_at, "publish = false")
    manifest.write_text("\n".join(lines), encoding="utf-8")
    return True


def main() -> int:
    changed = 0
    for manifest in workspace_members(ROOT):
        if add_publish_false(manifest):
            changed += 1
            print(f"marked private: {manifest.relative_to(ROOT)}")
    print(f"{changed} manifest(s) changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
