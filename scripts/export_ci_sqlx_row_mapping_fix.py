#!/usr/bin/env python3
"""Replace SQLx derive macros with explicit SQLite row mappings.

The generated implementations keep query_as ergonomics while removing the
sqlx-macros dependency chain that otherwise resolves unused MySQL/RSA crates.
This helper deletes itself after producing ordinary reviewable Rust code.
"""

from __future__ import annotations

import re
from pathlib import Path

DERIVE_RE = re.compile(r"#\[derive\((?P<body>.*?)\)\]", re.DOTALL)
STRUCT_RE = re.compile(
    r"(?P<visibility>pub(?:\([^)]*\))?\s+)?"
    r"struct\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*\{"
)
FIELD_RE = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?"
    r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*:"
    r"\s*(?P<type>.+?)\s*,?\s*(?://.*)?$"
)
EXPECTED_ROW_TYPES = 11


def matching_brace(text: str, opening: int) -> int:
    depth = 0
    in_string = False
    quote = ""
    escaped = False
    index = opening

    while index < len(text):
        char = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == quote:
                in_string = False
        else:
            if char == '"':
                in_string = True
                quote = char
            elif char == "{":
                depth += 1
            elif char == "}":
                depth -= 1
                if depth == 0:
                    return index
        index += 1

    raise SystemExit("unterminated struct while generating SQLx row mapping")


def parse_fields(path: Path, struct_name: str, body: str) -> list[str]:
    fields: list[str] = []
    for line in body.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("//"):
            continue
        if stripped.startswith("#["):
            raise SystemExit(
                f"{path}: {struct_name} has a field attribute that requires "
                "an explicit mapping decision"
            )
        match = FIELD_RE.match(line)
        if match is None:
            raise SystemExit(
                f"{path}: cannot safely parse {struct_name} field line {line!r}"
            )
        fields.append(match.group("name"))

    if not fields:
        raise SystemExit(f"{path}: {struct_name} has no named fields")
    return fields


def explicit_mapping(struct_name: str, fields: list[str]) -> str:
    assignments = "".join(
        f'            {field}: sqlx::Row::try_get(row, "{field}")?,\n'
        for field in fields
    )
    return (
        "\n\n"
        f"impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for {struct_name} {{\n"
        "    fn from_row(\n"
        "        row: &'r sqlx::sqlite::SqliteRow,\n"
        "    ) -> Result<Self, sqlx::Error> {\n"
        "        Ok(Self {\n"
        f"{assignments}"
        "        })\n"
        "    }\n"
        "}"
    )


def transform_rust_file(path: Path) -> list[str]:
    text = path.read_text(encoding="utf-8")
    replacements: list[tuple[int, int, str]] = []
    converted: list[str] = []

    for derive in list(DERIVE_RE.finditer(text)):
        items = [item.strip() for item in derive.group("body").split(",")]
        if "sqlx::FromRow" not in items:
            continue

        items.remove("sqlx::FromRow")
        if not items:
            raise SystemExit(f"{path}: refusing to create an empty derive attribute")

        struct = STRUCT_RE.search(text, derive.end())
        if struct is None:
            raise SystemExit(f"{path}: SQLx FromRow derive is not followed by a named struct")

        opening = text.find("{", struct.start(), struct.end())
        closing = matching_brace(text, opening)
        struct_name = struct.group("name")
        fields = parse_fields(path, struct_name, text[opening + 1 : closing])

        replacements.append(
            (
                derive.start(),
                derive.end(),
                "#[derive(" + ", ".join(items) + ")]",
            )
        )
        replacements.append(
            (closing + 1, closing + 1, explicit_mapping(struct_name, fields))
        )
        converted.append(f"{path}:{struct_name}")

    if replacements:
        for start, end, replacement in sorted(replacements, reverse=True):
            text = text[:start] + replacement + text[end:]
        path.write_text(text, encoding="utf-8")

    return converted


def remove_derive_feature() -> list[str]:
    updated: list[str] = []
    for path in sorted(Path(".").rglob("Cargo.toml")):
        text = path.read_text(encoding="utf-8")
        if "sqlx" not in text or '"derive"' not in text:
            continue

        original = text
        text = text.replace(', "derive"', "")
        text = text.replace('"derive", ', "")
        if text != original:
            path.write_text(text, encoding="utf-8")
            updated.append(str(path))

    if not updated:
        raise SystemExit("no SQLx derive feature declarations were removed")
    return updated


def main() -> None:
    converted: list[str] = []
    for path in sorted(Path("crates").rglob("*.rs")):
        converted.extend(transform_rust_file(path))

    if len(converted) != EXPECTED_ROW_TYPES:
        raise SystemExit(
            f"expected {EXPECTED_ROW_TYPES} SQLx row types, converted {len(converted)}: "
            + ", ".join(converted)
        )

    manifests = remove_derive_feature()
    print(f"Generated explicit SQLite mappings for {len(converted)} row types")
    print("Converted: " + ", ".join(converted))
    print("Removed SQLx derive feature from: " + ", ".join(manifests))
    Path("scripts/export_ci_sqlx_row_mapping_fix.py").unlink()


if __name__ == "__main__":
    main()
