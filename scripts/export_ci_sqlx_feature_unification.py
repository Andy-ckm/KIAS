#!/usr/bin/env python3
"""Normalize every SQLx dependency declaration across the workspace.

Workspace members must not silently re-enable SQLx default features because
those defaults pull macro, MySQL, PostgreSQL, and RSA dependencies that KIAS
never uses. The root workspace declaration remains the single feature source.
"""

from __future__ import annotations

import re
from pathlib import Path

SQLX_LINE_RE = re.compile(r"^(?P<indent>\s*)sqlx\s*=\s*(?P<spec>.+?)\s*$")
VERSION_RE = re.compile(r'version\s*=\s*"[^"]+"')
DEFAULT_RE = re.compile(r"default-features\s*=\s*(?:true|false)")


def normalize_inline_table(spec: str) -> str:
    if "workspace = true" in spec:
        # The root workspace declaration carries default-features=false.
        return spec

    if not spec.startswith("{") or not spec.endswith("}"):
        raise SystemExit(f"unsupported SQLx dependency syntax: {spec}")

    normalized = VERSION_RE.sub('version = "0.8.6"', spec, count=1)
    if "version" not in normalized:
        raise SystemExit(f"direct SQLx declaration is missing a version: {spec}")

    if DEFAULT_RE.search(normalized):
        normalized = DEFAULT_RE.sub("default-features = false", normalized, count=1)
    else:
        normalized = normalized.replace("{", "{ default-features = false,", 1)

    normalized = normalized.replace(', "derive"', "")
    normalized = normalized.replace('"derive", ', "")
    normalized = normalized.replace(', "macros"', "")
    normalized = normalized.replace('"macros", ', "")
    return normalized


def normalize_spec(spec: str) -> str:
    if spec.startswith('"') and spec.endswith('"'):
        return '{ version = "0.8.6", default-features = false }'
    return normalize_inline_table(spec)


def main() -> None:
    declarations: list[str] = []
    changed: list[str] = []

    for path in sorted(Path(".").rglob("Cargo.toml")):
        lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
        output: list[str] = []
        path_changed = False

        for number, line in enumerate(lines, start=1):
            newline = "\n" if line.endswith("\n") else ""
            content = line[:-1] if newline else line
            match = SQLX_LINE_RE.match(content)
            if match is None:
                output.append(line)
                continue

            spec = match.group("spec")
            normalized = normalize_spec(spec)
            replacement = f'{match.group("indent")}sqlx = {normalized}{newline}'
            output.append(replacement)
            declarations.append(f"{path}:{number}: sqlx = {normalized}")
            if replacement != line:
                path_changed = True

        if path_changed:
            path.write_text("".join(output), encoding="utf-8")
            changed.append(str(path))

    if not declarations:
        raise SystemExit("no SQLx dependency declarations found")

    root = Path("Cargo.toml").read_text(encoding="utf-8")
    root_sqlx = next(
        (line for line in root.splitlines() if line.strip().startswith("sqlx =")),
        "",
    )
    if "default-features = false" not in root_sqlx:
        raise SystemExit("workspace SQLx declaration does not disable default features")
    if '"derive"' in root_sqlx or '"macros"' in root_sqlx:
        raise SystemExit("workspace SQLx declaration still enables macro features")

    print(f"Validated {len(declarations)} SQLx declarations")
    for declaration in declarations:
        print(declaration)
    print("Changed manifests: " + (", ".join(changed) if changed else "none"))
    Path("scripts/export_ci_sqlx_feature_unification.py").unlink()


if __name__ == "__main__":
    main()
