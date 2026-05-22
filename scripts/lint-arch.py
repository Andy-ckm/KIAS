#!/usr/bin/env python3
"""
Architecture layer dependency checker using cargo metadata.
Replaces the grep-based approach with proper AST/dependency graph analysis.

Layer model:
  L0 (foundation): common, cache
  L1 (models): knowledge, skills, data-store, data-aggregator, monitor
  L2 (services): scheduler, controller, workflow-engine, team-engine, auto-loop,
                  goal-engine, autonomy-controller, model-router, llm-engine,
                  executor, agent-runtime, compliance-security, data-governance,
                  langgraph-engine, linux-automation, tool-executor, document-management,
                  it-change-management, mcp-protocol, im-integration, agent-view
  L3 (api): api-server, kias-main, kias-cli, benchmarks

Rules:
  - L0 crates must NOT depend on L1/L2/L3 crates
  - L1 crates may only depend on L0
  - L2 crates may depend on L0 and L1
  - L3 crates may depend on L0, L1, L2
"""

import json
import subprocess
import sys

LAYER_MAP = {
    # L0: Foundation
    "kias-common": 0, "kias-cache": 0,
    # L1: Models & Data
    "kias-knowledge": 1, "kias-skills": 1, "kias-data-store": 1,
    "kias-data-aggregator": 1, "kias-monitor": 1,
    # L2: Services
    "kias-scheduler": 2, "kias-controller": 2, "kias-workflow-engine": 2,
    "kias-team-engine": 2, "auto-loop": 2, "kias-goal-engine": 2,
    "kias-autonomy-controller": 2, "kias-model-router": 2, "llm-engine": 2,
    "kias-executor": 2, "agent-runtime": 2, "kias-compliance-security": 2,
    "kias-data-governance": 2, "kias-langgraph-engine": 2,
    "kias-linux-automation": 2, "tool-executor": 2,
    "document-management": 2, "it-change-management": 2,
    "mcp-protocol": 2, "im-integration": 2, "kias-agent-view": 2,
    # L3: API
    "kias-api-server": 3, "kias-main": 3, "kias-cli": 3,
    "benchmarks": 3, "kias-benchmarks": 3,
}


def get_workspace_deps():
    """Get workspace dependency graph using cargo metadata."""
    r = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        capture_output=True, text=True, timeout=60,
        cwd="/workspace/kias"
    )
    if r.returncode != 0:
        print(f"ERROR: cargo metadata failed: {r.stderr}")
        sys.exit(1)

    meta = json.loads(r.stdout)
    workspace_members = {p["name"] for p in meta["packages"]}

    deps = {}
    for pkg in meta["packages"]:
        name = pkg["name"]
        if name not in workspace_members:
            continue
        pkg_deps = set()
        for dep in pkg["dependencies"]:
            if dep["name"] in workspace_members and not dep.get("optional", False):
                pkg_deps.add(dep["name"])
        deps[name] = pkg_deps

    return deps


def check_violations(deps):
    """Check for layer violations."""
    violations = []

    for pkg, pkg_deps in deps.items():
        pkg_layer = LAYER_MAP.get(pkg)
        if pkg_layer is None:
            # Unknown crate - skip
            continue

        for dep in pkg_deps:
            dep_layer = LAYER_MAP.get(dep)
            if dep_layer is None:
                continue

            if dep_layer > pkg_layer:
                violations.append(
                    f"  LAYER VIOLATION: {pkg} (L{pkg_layer}) depends on {dep} (L{dep_layer})"
                )

    return violations


def main():
    print("Checking architecture layers (cargo metadata)...")

    deps = get_workspace_deps()
    print(f"  Workspace crates: {len(deps)}")

    violations = check_violations(deps)

    if violations:
        print(f"\nERROR: {len(violations)} architecture layer violation(s):")
        for v in violations:
            print(v)
        sys.exit(1)
    else:
        print("  ✔ Architecture layers OK")
        sys.exit(0)


if __name__ == "__main__":
    main()
