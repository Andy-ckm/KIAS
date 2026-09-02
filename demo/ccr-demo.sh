#!/usr/bin/env bash
# Compatibility wrapper for the machine-verifiable controlled-change harness.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCENARIO="${1:-happy}"
OUTPUT="${2:-/tmp/kias-controlled-change-${SCENARIO}.json}"

python3 "$ROOT/demo/controlled_change_harness.py" \
  --scenario "$SCENARIO" \
  --output "$OUTPUT" >/dev/null

python3 - "$OUTPUT" <<'PY'
import json, sys
p=json.load(open(sys.argv[1]))
print("Controlled Change Harness")
print(f"scenario={p['scenario']} state={p['state']}")
print(f"events={len(p['events'])} checkpoints={len(p['checkpoints'])}")
print(f"verification_passed={p['verification']['passed']}")
print(f"report_hash={p['report_hash']}")
print(f"evidence_file={sys.argv[1]}")
print("non_claims=" + "; ".join(p['non_claims']))
PY
