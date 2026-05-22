#!/usr/bin/env bash
# =============================================================================
# lint-arch-v2.sh — Architecture layer dependency validation using cargo metadata
#
# Validates the L0 → L1 → L2 → L3 dependency rules programmatically by
# extracting the actual dependency graph from `cargo metadata`.
#
# Layer definitions (from AGENTS.md):
#   L0: common, cache, monitor              — foundation, no internal deps beyond self
#   L1: knowledge, skills, data-store, data-governance  — depend on L0 only
#   L2: scheduler, controller, executor, workflow-engine, team-engine,
#       goal-engine, autonomy-controller, mcp-protocol, langgraph-engine,
#       model-router, llm-engine, tool-executor, agent-runtime, auto-loop,
#       data-aggregator, im-integration, compliance-security,
#       linux-automation, it-change-management, document-management,
#       harness-registry, a2a-registry
#   L3: api-server, kias-main, kias-cli, agent-view     — depend on L0+L1+L2
#
# Rules:
#   - L0 crates must NOT depend on L1/L2/L3 crates
#   - L1 crates must NOT depend on L2/L3 crates
#   - L2 crates must NOT depend on L3 crates
#   - Any violation is an error
#
# Usage:
#   ./scripts/lint-arch-v2.sh
#   ./scripts/lint-arch-v2.sh --verbose
# =============================================================================
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
RESET='\033[0m'

VERBOSE="${1:-}"

# ── Layer definitions ───────────────────────────────────────────────────────

# L0: Foundation crates (no internal deps beyond self)
L0_CRATES=("kias-common" "kias-cache" "kias-monitor")

# L1: Data & knowledge crates (depend on L0 only)
L1_CRATES=("kias-knowledge" "kias-skills" "kias-data-store" "kias-data-governance")

# L2: Service crates (depend on L0 + L1)
L2_CRATES=(
    "kias-scheduler" "kias-controller" "kias-executor"
    "kias-workflow-engine" "kias-team-engine" "kias-goal-engine"
    "kias-autonomy-controller" "kias-mcp-protocol" "kias-langgraph-engine"
    "kias-model-router" "kias-llm-engine" "kias-tool-executor"
    "kias-agent-runtime" "auto-loop" "kias-data-aggregator"
    "kias-im-integration" "kias-compliance-security"
    "kias-linux-automation" "kias-it-change-management"
    "kias-document-management" "kias-harness-registry" "kias-a2a-registry"
)

# L3: API / orchestration crates (depend on L0 + L1 + L2)
L3_CRATES=("kias-api-server" "kias-main" "kias-cli" "kias-agent-view")

ALL_KIAS_CRATES=("${L0_CRATES[@]}" "${L1_CRATES[@]}" "${L2_CRATES[@]}" "${L3_CRATES[@]}")

# ── Extract dependency graph from cargo metadata ────────────────────────────

printf "${BLUE}${BOLD}▶ Extracting dependency graph via cargo metadata...${RESET}\n"

METADATA=$(cargo metadata --format-version 1 --no-deps 2>/dev/null)

if [ -z "$METADATA" ]; then
    printf "${RED}✘ Failed to run cargo metadata${RESET}\n"
    exit 1
fi

# Extract internal dependency map: for each workspace crate, list which other
# workspace crates it depends on.
# We use a JSON-aware approach: iterate over workspace members and their deps.
declare -A CRATE_DEPS  # key="crate_name" value="dep1 dep2 dep3 ..."

# Parse workspace members and their dependencies
while IFS= read -r pkg_name; do
    # Get the dependencies for this package that are also workspace members
    deps=$(echo "$METADATA" | python3 -c "
import json, sys
data = json.load(sys.stdin)
target = sys.argv[1]
ws_member_ids = {p['id'] for p in data['packages'] if any(m.endswith('/' + p['name'].replace('-', '_')) or m.endswith('/' + p['name']) for m in data['workspace_members'])}
# Also build name→id mapping
name_to_id = {p['name']: p['id'] for p in data['packages']}
for pkg in data['packages']:
    if pkg['name'] == target:
        internal_deps = []
        for dep in pkg['dependencies']:
            if dep['name'] in name_to_id and name_to_id[dep['name']] in ws_member_ids:
                internal_deps.append(dep['name'])
        print(' '.join(sorted(internal_deps)))
        break
" "$pkg_name" 2>/dev/null)
    CRATE_DEPS["$pkg_name"]="$deps"
    if [ "$VERBOSE" = "--verbose" ]; then
        printf "  %-35s → %s\n" "$pkg_name" "${deps:-(none)}"
    fi
done < <(echo "$METADATA" | python3 -c "
import json, sys
data = json.load(sys.stdin)
ws_member_ids = {p['id'] for p in data['packages'] if any(m.endswith('/' + p['name'].replace('-', '_')) or m.endswith('/' + p['name']) for m in data['workspace_members'])}
for p in sorted(data['packages'], key=lambda x: x['name']):
    if p['id'] in ws_member_ids:
        print(p['name'])
")

# ── Helper: get layer of a crate ───────────────────────────────────────────

get_layer() {
    local crate_name="$1"
    for c in "${L0_CRATES[@]}"; do [[ "$c" == "$crate_name" ]] && echo 0 && return; done
    for c in "${L1_CRATES[@]}"; do [[ "$c" == "$crate_name" ]] && echo 1 && return; done
    for c in "${L2_CRATES[@]}"; do [[ "$c" == "$crate_name" ]] && echo 2 && return; done
    for c in "${L3_CRATES[@]}"; do [[ "$c" == "$crate_name" ]] && echo 3 && return; done
    echo -1  # unknown crate
}

# ── Validate dependencies ──────────────────────────────────────────────────

ERRORS=0
CHECKED=0

printf "\n${BLUE}${BOLD}▶ Validating layer dependencies...${RESET}\n\n"

for crate_name in "${ALL_KIAS_CRATES[@]}"; do
    crate_layer=$(get_layer "$crate_name")
    if [ "$crate_layer" -eq -1 ]; then
        continue  # skip unknown
    fi

    deps="${CRATE_DEPS[$crate_name]:-}"

    if [ -z "$deps" ]; then
        continue  # no internal deps
    fi

    for dep in $deps; do
        dep_layer=$(get_layer "$dep")
        if [ "$dep_layer" -eq -1 ]; then
            continue  # external or unknown dep, skip
        fi

        ((CHECKED++))

        # Rule: a crate at layer N must NOT depend on a crate at layer > N
        # Allowed: same layer, or lower layer
        if [ "$dep_layer" -gt "$crate_layer" ]; then
            printf "${RED}✘ VIOLATION: L%d %-30s → L%d %-30s (upward dependency!)${RESET}\n" \
                "$crate_layer" "$crate_name" "$dep_layer" "$dep"
            ((ERRORS++))
        elif [ "$dep_layer" -eq "$crate_layer" ]; then
            if [ "$VERBOSE" = "--verbose" ]; then
                printf "${YELLOW}  ⚠ Same-layer: L%d %-30s ↔ L%d %-30s${RESET}\n" \
                    "$crate_layer" "$crate_name" "$dep_layer" "$dep"
            fi
        else
            if [ "$VERBOSE" = "--verbose" ]; then
                printf "${GREEN}  ✔ L%d %-30s → L%d %-30s${RESET}\n" \
                    "$crate_layer" "$crate_name" "$dep_layer" "$dep"
            fi
        fi
    done
done

# ── Summary ────────────────────────────────────────────────────────────────

printf "\n${BOLD}══════ Architecture Lint Summary ══════${RESET}\n"
printf "Crates defined:  %d\n" "${#ALL_KIAS_CRATES[@]}"
printf "Deps checked:    %d\n" "$CHECKED"

if [ "$ERRORS" -gt 0 ]; then
    printf "${RED}${BOLD}✘ FAILED: %d architecture layer violations detected${RESET}\n" "$ERRORS"
    printf "\n${YELLOW}Fix: ensure lower-layer crates are not imported by higher-layer ones.${RESET}\n"
    printf "  L0 (common, cache, monitor)         ← no internal deps\n"
    printf "  L1 (knowledge, skills, data-store)   ← may depend on L0 only\n"
    printf "  L2 (scheduler, controller, ...)      ← may depend on L0+L1\n"
    printf "  L3 (api-server, kias-main, ...)      ← may depend on L0+L1+L2\n"
    exit 1
fi

printf "${GREEN}${BOLD}✔ All architecture layer rules satisfied${RESET}\n"
exit 0
