#!/usr/bin/env python3
"""Align every rusqlite manifest with SQLx's SQLite binding, then self-delete."""

from pathlib import Path

old = 'rusqlite = { version = "0.31", features = ["bundled"] }'
new = 'rusqlite = { version = "0.32.1", features = ["bundled"] }'
updated_declarations = 0
updated_manifests = []

for path in sorted(Path(".").rglob("Cargo.toml")):
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count == 0:
        continue
    path.write_text(text.replace(old, new), encoding="utf-8")
    updated_declarations += count
    updated_manifests.append(str(path))

if updated_declarations < 2:
    raise SystemExit(
        "expected at least the workspace and document-management rusqlite declarations; "
        f"updated {updated_declarations}"
    )

print(
    f"Aligned {updated_declarations} rusqlite declarations across "
    f"{len(updated_manifests)} manifests: {', '.join(updated_manifests)}"
)
Path("scripts/export_ci_sqlite_binding_fix.py").unlink()
