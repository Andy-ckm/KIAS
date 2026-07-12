#!/usr/bin/env python3
"""Apply the remaining strict Clippy fixes, then self-delete."""

from pathlib import Path

path = Path("crates/kias-main/src/services/init.rs")
text = path.read_text(encoding="utf-8")
replacements = {
    "token.as_bytes().len() < MIN_STATIC_TOKEN_BYTES": "token.len() < MIN_STATIC_TOKEN_BYTES",
    "secret.as_bytes().len() < MIN_JWT_SECRET_BYTES": "secret.len() < MIN_JWT_SECRET_BYTES",
}
for old, new in replacements.items():
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one match for {old!r}, found {count}")
    text = text.replace(old, new, 1)
path.write_text(text, encoding="utf-8")
Path("scripts/export_ci_clippy_fix.py").unlink()
