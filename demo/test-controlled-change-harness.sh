#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

for scenario in happy approval-bypass evidence-missing; do
  python3 "$ROOT/demo/controlled_change_harness.py" \
    --scenario "$scenario" \
    --output "$TMP_DIR/$scenario.json" >/dev/null
done

python3 - "$TMP_DIR" <<'PY'
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
reports = {name: json.loads((root / f"{name}.json").read_text()) for name in (
    "happy", "approval-bypass", "evidence-missing"
)}

happy = reports["happy"]
assert happy["state"] == "closed"
assert happy["verification"]["passed"] is True
assert any(event["event_type"] == "duplicate_side_effect_blocked" for event in happy["events"])

for scenario, expected in (
    ("approval-bypass", "separation_of_duties_violation"),
    ("evidence-missing", "missing_evidence"),
):
    report = reports[scenario]
    assert report["state"] == "awaiting_approval"
    assert report["expected_failure"] == expected
    assert report["verification"]["passed"] is True
    denied = [event for event in report["events"] if event["event_type"] == "policy_denied"]
    assert denied and expected in denied[-1]["payload"]["reason"]

for report in reports.values():
    assert len(report["report_hash"]) == 64
    assert report["non_claims"]

print("controlled-change-harness: 3 scenarios passed")
PY
