#!/usr/bin/env python3
"""Enforce KIAS product-tier dependency boundaries.

The repository may contain Core, Extensions, and Labs in one Cargo workspace, but
Core crates must never acquire a normal, development, or build dependency on a
Labs crate. The default Cargo surface must also contain Core crates only.
"""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]

CORE_PATHS = {
    "crates/common",
    "crates/controller",
    "crates/scheduler",
    "crates/workflow-engine",
    "crates/autonomy-controller",
    "crates/executor",
    "crates/tool-executor",
    "crates/agent-runtime",
    "crates/data-store",
    "crates/data-governance",
    "crates/monitor",
    "crates/model-router",
    "crates/compliance-security",
    "crates/api-server",
    "crates/kias-main",
    "crates/kias-cli",
}

LAB_PACKAGE_NAMES = {
    "auto-loop",
    "data-aggregator",
    "im-integration",
    "it-change-management",
    "linux-automation",
    "gxp-compliance",
    "kias-goal-engine",
}

DEPENDENCY_TABLES = (
    "dependencies",
    "dev-dependencies",
    "build-dependencies",
)


def load_toml(path: Path) -> dict[str, Any]:
    with path.open("rb") as source:
        return tomllib.load(source)


def dependency_package_name(key: str, specification: Any) -> str:
    if isinstance(specification, dict):
        package = specification.get("package")
        if isinstance(package, str):
            return package
    return key


def source_references(crate_path: Path, dependency_name: str) -> list[str]:
    rust_name = dependency_name.replace("-", "_")
    references: list[str] = []
    source_root = crate_path / "src"
    if not source_root.exists():
        return references

    for source in sorted(source_root.rglob("*.rs")):
        for line_number, line in enumerate(
            source.read_text("utf-8", errors="replace").splitlines(), 1
        ):
            if rust_name in line:
                relative = source.relative_to(ROOT)
                references.append(f"  {relative}:{line_number}: {line.strip()[:180]}")
                if len(references) >= 12:
                    return references
    return references


def check_default_members(workspace: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    default_members = workspace.get("workspace", {}).get("default-members", [])
    if not default_members:
        return ["Cargo.toml must define workspace.default-members for the Core surface"]

    for member in default_members:
        if member not in CORE_PATHS:
            errors.append(f"default member is not classified as Core: {member}")
    return errors


def check_core_dependencies() -> list[str]:
    errors: list[str] = []

    for relative_path in sorted(CORE_PATHS):
        crate_path = ROOT / relative_path
        manifest_path = crate_path / "Cargo.toml"
        if not manifest_path.exists():
            errors.append(f"classified Core manifest is missing: {relative_path}/Cargo.toml")
            continue

        manifest = load_toml(manifest_path)
        for table_name in DEPENDENCY_TABLES:
            dependencies = manifest.get(table_name, {})
            if not isinstance(dependencies, dict):
                continue

            for key, specification in dependencies.items():
                package_name = dependency_package_name(key, specification)
                if package_name not in LAB_PACKAGE_NAMES and key not in LAB_PACKAGE_NAMES:
                    continue

                dependency_name = package_name if package_name in LAB_PACKAGE_NAMES else key
                errors.append(
                    f"{relative_path} has forbidden {table_name} dependency on Labs crate "
                    f"{dependency_name!r}"
                )
                errors.extend(source_references(crate_path, dependency_name))

    return errors


def main() -> int:
    workspace = load_toml(ROOT / "Cargo.toml")
    errors = check_default_members(workspace)
    errors.extend(check_core_dependencies())

    if errors:
        print("KIAS architecture boundary violations:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("Architecture boundary check passed: default surface is Core-only and Core has no Labs dependencies.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())