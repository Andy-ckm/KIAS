#!/usr/bin/env python3
"""Align rusqlite with SQLx's SQLite native binding, then self-delete."""

from pathlib import Path

TARGETS = (
    "Cargo.toml",
    "crates/document-management/Cargo.toml",
)

for path_name in TARGETS:
    path = Path(path_name)
    text = path.read_text(encoding="utf-8")
    old = 'rusqlite = { version = "0.31", features = ["bundled"] }'
    new = 'rusqlite = { version = "0.32.1", features = ["bundled"] }'
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path_name}: expected one rusqlite 0.31 declaration, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")

Path("scripts/export_ci_sqlite_binding_fix.py").unlink()
