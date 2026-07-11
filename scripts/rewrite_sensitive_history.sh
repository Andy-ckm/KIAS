#!/usr/bin/env bash
set -euo pipefail

# This script rewrites Git history. Run it only from a fresh mirror clone after
# credentials have been revoked/rotated and collaborators have stopped pushing.
# It defaults to a dry-run plan. Pass --execute to perform the rewrite.

MODE="${1:-}"

PATHS=(
  ".dev-state.yaml"
  ".dev-log"
  ".dev-insights.md"
  ".dev-tasks.yaml"
  ".goal-state.yaml"
  ".orchestrator-state.json"
  ".research-queue.yaml"
  ".task-queue"
  "kias/.task-queue"
  ".trace"
  "docs/status/安全扫描报告-2026-05-23.md"
  "docs/papers"
  "docs/reference"
)

printf '%s\n' "KIAS history-cleanup plan" "Repository: $(pwd)" "Paths to remove from every reachable revision:"
printf '  - %s\n' "${PATHS[@]}"

if [[ "$MODE" != "--execute" ]]; then
  cat <<'EOF'

Dry run only. No history was changed.

Required preparation:
  1. Revoke or rotate every credential ever used with this project.
  2. Make a protected offline backup of the repository.
  3. Pause pushes and close/record open pull-request state.
  4. Create a fresh mirror clone:
       git clone --mirror https://github.com/Andy-ckm/KIAS.git KIAS-cleanup.git
       cd KIAS-cleanup.git
  5. Install git-filter-repo and rerun this script with --execute.

History cleanup does not remove copies from forks, old clones, caches, logs,
release artifacts, package registries, or screenshots.
EOF
  exit 0
fi

command -v git-filter-repo >/dev/null 2>&1 || {
  echo "git-filter-repo is required" >&2
  exit 2
}

[[ "$(git rev-parse --is-bare-repository)" == "true" ]] || {
  echo "Refusing to run: use a fresh --mirror clone" >&2
  exit 2
}

[[ -z "$(git status --porcelain 2>/dev/null || true)" ]] || {
  echo "Refusing to run: repository is not clean" >&2
  exit 2
}

FILTER_ARGS=(--force --invert-paths)
for path in "${PATHS[@]}"; do
  FILTER_ARGS+=(--path "$path")
done

git filter-repo "${FILTER_ARGS[@]}"

echo "History rewritten locally. Review before force-pushing:"
echo "  git log --all --stat --oneline"
echo "  git rev-list --objects --all | grep -E '(task-queue|dev-log|安全扫描报告|docs/papers|docs/reference)' && exit 1 || true"
echo "  trufflehog git file://\"$(pwd)\" --only-verified --fail"
echo
echo "After review, force-push all branches and tags intentionally:"
echo "  git push --force --mirror origin"
echo
echo "Then expire old GitHub objects where possible, rebuild releases, and require every collaborator to re-clone."
