#!/usr/bin/env python3
"""Pin the maintained SQLx 0.8 patch line and remove this helper."""

from pathlib import Path

MANIFESTS = (
    "Cargo.toml",
    "crates/data-store/Cargo.toml",
    "crates/data-governance/Cargo.toml",
    "crates/api-server/Cargo.toml",
)

for path_name in MANIFESTS:
    path = Path(path_name)
    text = path.read_text(encoding="utf-8")
    old = 'sqlx = { version = "0.8", default-features = false,'
    new = 'sqlx = { version = "0.8.6", default-features = false,'
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path_name}: expected one SQLx 0.8 declaration, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")

Path("scripts/export_ci_sqlx_version_fix.py").unlink()
