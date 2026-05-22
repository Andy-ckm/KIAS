#!/usr/bin/env bash
# =============================================================================
# ci-smoke.sh — Quick CI smoke test for KIAS monorepo
#
# Runs the minimum checks to catch regressions before merging:
#   1. cargo fmt --check        (formatting)
#   2. cargo clippy -- -D warnings  (lint)
#   3. cargo test --workspace    (all unit + integration tests)
#   4. curl health check         (runtime smoke against live server)
#
# Usage:
#   ./scripts/ci-smoke.sh                  # default: start server, run checks
#   SKIP_SERVER=1 ./scripts/ci-smoke.sh    # skip health check (server not running)
#   HEALTH_URL=http://host:9090/health ./scripts/ci-smoke.sh  # custom endpoint
# =============================================================================
set -euo pipefail

# ── Config ──────────────────────────────────────────────────────────────────
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

HEALTH_URL="${HEALTH_URL:-http://localhost:8080/health}"
HEALTH_TIMEOUT="${HEALTH_TIMEOUT:-30}"
SKIP_SERVER="${SKIP_SERVER:-0}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
RESET='\033[0m'

PASS=0
FAIL=0

log_step() {
    printf "\n${BLUE}${BOLD}══════ %s ══════${RESET}\n" "$1"
}

pass_step() {
    printf "${GREEN}✔ %s${RESET}\n" "$1"
    ((PASS++))
}

fail_step() {
    printf "${RED}✘ %s${RESET}\n" "$1"
    ((FAIL++))
}

# ── Step 1: cargo fmt --check ───────────────────────────────────────────────
log_step "1/4  cargo fmt --check"
if cargo fmt --check 2>&1; then
    pass_step "Formatting OK"
else
    fail_step "Formatting issues detected — run 'cargo fmt' to fix"
fi

# ── Step 2: cargo clippy ────────────────────────────────────────────────────
log_step "2/4  cargo clippy -- -D warnings"
if cargo clippy -- -D warnings 2>&1; then
    pass_step "Clippy OK (no warnings)"
else
    fail_step "Clippy warnings/errors found"
fi

# ── Step 3: cargo test --workspace ──────────────────────────────────────────
log_step "3/4  cargo test --workspace"
if cargo test --workspace 2>&1; then
    pass_step "All tests passed"
else
    fail_step "Test failures detected"
fi

# ── Step 4: Health check (curl) ─────────────────────────────────────────────
log_step "4/4  Health check ($HEALTH_URL)"
if [ "$SKIP_SERVER" = "1" ]; then
    printf "${YELLOW}⏭  Skipped (SKIP_SERVER=1)${RESET}\n"
else
    # Wait for server to become ready (up to HEALTH_TIMEOUT seconds)
    READY=0
    SECONDS_WAITED=0
    printf "Waiting for server at %s (timeout: %ds)...\n" "$HEALTH_URL" "$HEALTH_TIMEOUT"
    while [ "$SECONDS_WAITED" -lt "$HEALTH_TIMEOUT" ]; do
        if curl -sf --max-time 2 "$HEALTH_URL" > /dev/null 2>&1; then
            READY=1
            break
        fi
        sleep 1
        ((SECONDS_WAITED++))
    done

    if [ "$READY" -eq 1 ]; then
        # Fetch and validate JSON response
        HEALTH_BODY=$(curl -sf --max-time 5 "$HEALTH_URL" 2>/dev/null)
        if echo "$HEALTH_BODY" | grep -q '"status"'; then
            pass_step "Health check OK: $HEALTH_BODY"
        else
            fail_step "Health check returned unexpected body: $HEALTH_BODY"
        fi
    else
        fail_step "Health check failed — server not reachable at $HEALTH_URL within ${HEALTH_TIMEOUT}s"
    fi
fi

# ── Summary ─────────────────────────────────────────────────────────────────
printf "\n${BOLD}══════ CI Smoke Summary ══════${RESET}\n"
printf "${GREEN}Passed: %d${RESET}  " "$PASS"
printf "${RED}Failed: %d${RESET}\n" "$FAIL"

if [ "$FAIL" -gt 0 ]; then
    printf "\n${RED}${BOLD}CI SMOKE FAILED${RESET}\n"
    exit 1
fi

printf "\n${GREEN}${BOLD}CI SMOKE PASSED${RESET}\n"
exit 0
