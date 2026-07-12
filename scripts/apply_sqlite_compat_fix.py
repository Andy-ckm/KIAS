#!/usr/bin/env python3
"""Align rusqlite with the libsqlite3-sys version required by repaired SQLx."""

from pathlib import Path


def replace_exact(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one match, found {count}: {old!r}")
    target.write_text(text.replace(old, new), encoding="utf-8")


replace_exact(
    "Cargo.toml",
    'rusqlite = { version = "0.31", features = ["bundled"] }',
    'rusqlite = { version = "0.32", features = ["bundled"] }',
)
replace_exact(
    "crates/document-management/Cargo.toml",
    'rusqlite = { version = "0.31", features = ["bundled"] }',
    'rusqlite.workspace = true',
)
