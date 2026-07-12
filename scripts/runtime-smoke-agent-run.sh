#!/usr/bin/env bash
set -euo pipefail
exec > >(tee runtime-lifecycle.log) 2>&1

wait_for_health() {
  for attempt in $(seq 1 80); do
    if curl --fail --silent http://127.0.0.1:8080/health >/dev/null; then
      return 0
    fi
    if ! kill -0 "${SERVER_PID}" 2>/dev/null; then
      echo "KIAS exited before becoming healthy" >&2
      cat runtime-smoke.log >&2
      exit 1
    fi
    if [[ "${attempt}" -eq 80 ]]; then
      echo "KIAS did not become healthy" >&2
      cat runtime-smoke.log >&2
      exit 1
    fi
    sleep 0.25
  done
}

start_server() {
  target/debug/kias server >>runtime-smoke.log 2>&1 &
  SERVER_PID=$!
  wait_for_health
}

stop_server() {
  kill -TERM "${SERVER_PID}"
  wait "${SERVER_PID}"
  SERVER_PID=""
}

api() {
  curl --fail-with-body --silent --show-error \
    -H "Authorization: Bearer ${TOKEN}" \
    "$@"
}

wait_for_run() {
  local run_id="$1"
  local expected="$2"
  for attempt in $(seq 1 160); do
    local payload
    payload="$(api "http://127.0.0.1:8080/api/v1/runs/${run_id}")"
    local status
    status="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["status"])' <<<"${payload}")"
    if [[ "${status}" == "${expected}" ]]; then
      printf '%s\n' "${payload}"
      return 0
    fi
    if [[ "${status}" =~ ^(succeeded|failed|cancelled|interrupted)$ ]]; then
      echo "Run ${run_id} reached ${status}; expected ${expected}" >&2
      printf '%s\n' "${payload}" >&2
      return 1
    fi
    if [[ "${attempt}" -eq 160 ]]; then
      echo "Run ${run_id} did not reach ${expected}" >&2
      return 1
    fi
    sleep 0.25
  done
}

create_agent() {
  local key="$1"
  local body="$2"
  api -X POST -H "Content-Type: application/json" \
    -H "X-Idempotency-Key: ${key}" --data "${body}" \
    http://127.0.0.1:8080/api/v1/agents
}

start_run() {
  local agent_id="$1"
  local key="$2"
  local body="$3"
  api -X POST -H "Content-Type: application/json" \
    -H "X-Idempotency-Key: ${key}" --data "${body}" \
    "http://127.0.0.1:8080/api/v1/agents/${agent_id}/runs"
}

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill -TERM "${SERVER_PID}" 2>/dev/null || true
    sleep 1
    kill -KILL "${SERVER_PID}" 2>/dev/null || true
  fi
  docker ps -aq --filter label=kias.run_id | xargs -r docker rm --force >/dev/null 2>&1 || true
}
trap cleanup EXIT

: >runtime-smoke.log
start_server
TOKEN="$(target/debug/kias token --role operator --subject runtime-smoke)"

echo "stage=capabilities"
CAPABILITIES="$(api http://127.0.0.1:8080/api/v1/system/capabilities)"
printf 'capabilities=%s\n' "${CAPABILITIES}"
python3 -c '
import json,sys
payload=json.load(sys.stdin)
capabilities={item["id"]:item for item in payload["capabilities"]}
assert payload["profile"] == "core"
assert capabilities["sandboxed-runs"]["enabled"] is True
' <<<"${CAPABILITIES}"

SUCCESS_AGENT="$(create_agent runtime-success-agent \
  '{"name":"runtime-success","image":"busybox:1.36","command":["sh","-c","cat"],"resource_request":{"cpu":"500m","memory":"64Mi","gpu":"0"},"labels":{"kias.io/execution":"enabled"}}')"
SUCCESS_AGENT_ID="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["id"])' <<<"${SUCCESS_AGENT}")"

echo "stage=success-run-admission"
SUCCESS_RUN="$(start_run "${SUCCESS_AGENT_ID}" runtime-success-run \
  '{"input":"hello from a bounded KIAS Agent Run","timeout_seconds":20,"max_retries":0}')"
printf 'success_run=%s\n' "${SUCCESS_RUN}"
SUCCESS_RUN_ID="$(python3 -c 'import json,sys; payload=json.load(sys.stdin); assert payload["policy"]["allowed"] is True; print(payload["id"])' <<<"${SUCCESS_RUN}")"
wait_for_run "${SUCCESS_RUN_ID}" succeeded >/dev/null

echo "stage=success-run-logs"
LOGS="$(api "http://127.0.0.1:8080/api/v1/runs/${SUCCESS_RUN_ID}/logs")"
printf 'logs=%s\n' "${LOGS}"
python3 -c 'import json,sys; assert "hello from a bounded KIAS Agent Run" in json.load(sys.stdin)["stdout"]' <<<"${LOGS}"

echo "stage=success-run-evidence"
EVIDENCE="$(api "http://127.0.0.1:8080/api/v1/runs/${SUCCESS_RUN_ID}/evidence")"
printf 'evidence=%s\n' "${EVIDENCE}"
python3 -c '
import json,sys
evidence=json.load(sys.stdin)
assert len(evidence["evidence_sha256"]) == 64
assert evidence["run"]["policy"]["allowed"] is True
assert evidence["final_execution"]["sandbox"]["network"] == "none"
assert evidence["final_execution"]["sandbox"]["root_filesystem"] == "read-only"
assert evidence["final_execution"]["sandbox"]["host_mounts"] is False
assert evidence["final_execution"]["sandbox"]["no_new_privileges"] is True
assert evidence["final_execution"]["resource_usage"]["configured_memory_bytes"] == 67108864
' <<<"${EVIDENCE}"

CHECKPOINT="$(api "http://127.0.0.1:8080/api/v1/runs/${SUCCESS_RUN_ID}/checkpoint")"
python3 -c 'import json,sys; payload=json.load(sys.stdin); assert payload["replayable"] is True; assert len(payload["agent_spec_sha256"]) == 64' <<<"${CHECKPOINT}"

RUN_ID="${SUCCESS_RUN_ID}" python3 - <<'PY'
import os
import sqlite3
database = sqlite3.connect("runtime-smoke.db")
stored = database.execute("SELECT input FROM tasks WHERE id = ?", (os.environ["RUN_ID"],)).fetchone()[0]
assert "hello from a bounded KIAS Agent Run" not in stored
assert "input_sha256" in stored
PY

FAIL_AGENT="$(create_agent runtime-fail-agent \
  '{"name":"runtime-fail","image":"busybox:1.36","command":["sh","-c","exit 7"],"labels":{"kias.io/execution":"enabled"}}')"
FAIL_AGENT_ID="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["id"])' <<<"${FAIL_AGENT}")"
FAIL_RUN="$(start_run "${FAIL_AGENT_ID}" runtime-fail-run '{"max_retries":1}')"
FAIL_RUN_ID="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])' <<<"${FAIL_RUN}")"
FAILED="$(wait_for_run "${FAIL_RUN_ID}" failed)"
python3 -c 'import json,sys; assert json.load(sys.stdin)["retry_count"] == 1' <<<"${FAILED}"
FAIL_EVIDENCE="$(api "http://127.0.0.1:8080/api/v1/runs/${FAIL_RUN_ID}/evidence")"
python3 -c 'import json,sys; assert len(json.load(sys.stdin)["attempts"]) == 2' <<<"${FAIL_EVIDENCE}"

RETRY_RUN="$(api -X POST -H "Content-Type: application/json" \
  -H "X-Idempotency-Key: runtime-retry-run" --data '{}' \
  "http://127.0.0.1:8080/api/v1/runs/${FAIL_RUN_ID}/retry")"
RETRY_RUN_ID="$(python3 -c 'import json,sys; payload=json.load(sys.stdin); assert payload["lineage"]["retry_of"]; print(payload["id"])' <<<"${RETRY_RUN}")"
wait_for_run "${RETRY_RUN_ID}" failed >/dev/null

LONG_AGENT="$(create_agent runtime-long-agent \
  '{"name":"runtime-long","image":"busybox:1.36","command":["sh","-c","sleep 60"],"labels":{"kias.io/execution":"enabled"}}')"
LONG_AGENT_ID="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["id"])' <<<"${LONG_AGENT}")"

CANCEL_RUN="$(start_run "${LONG_AGENT_ID}" runtime-cancel-run '{"timeout_seconds":120}')"
CANCEL_RUN_ID="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])' <<<"${CANCEL_RUN}")"
wait_for_run "${CANCEL_RUN_ID}" running >/dev/null
CANCELLED="$(api -X POST -H "X-Idempotency-Key: runtime-cancel-action" "http://127.0.0.1:8080/api/v1/runs/${CANCEL_RUN_ID}/cancel")"
python3 -c 'import json,sys; assert json.load(sys.stdin)["status"] == "cancelled"' <<<"${CANCELLED}"

INTERRUPTED_RUN="$(start_run "${LONG_AGENT_ID}" runtime-interrupted-run '{"timeout_seconds":120}')"
INTERRUPTED_RUN_ID="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])' <<<"${INTERRUPTED_RUN}")"
wait_for_run "${INTERRUPTED_RUN_ID}" running >/dev/null
stop_server
start_server

INTERRUPTED="$(api "http://127.0.0.1:8080/api/v1/runs/${INTERRUPTED_RUN_ID}")"
python3 -c 'import json,sys; assert json.load(sys.stdin)["status"] == "interrupted"' <<<"${INTERRUPTED}"
RECOVERED="$(api -X POST -H "Content-Type: application/json" \
  -H "X-Idempotency-Key: runtime-recover-run" --data '{}' \
  "http://127.0.0.1:8080/api/v1/runs/${INTERRUPTED_RUN_ID}/recover")"
RECOVERED_ID="$(python3 -c 'import json,sys; payload=json.load(sys.stdin); assert payload["lineage"]["recovery_of"]; print(payload["id"])' <<<"${RECOVERED}")"
wait_for_run "${RECOVERED_ID}" running >/dev/null
api -X POST -H "X-Idempotency-Key: runtime-cancel-recovered" "http://127.0.0.1:8080/api/v1/runs/${RECOVERED_ID}/cancel" >/dev/null

for agent_id in "${SUCCESS_AGENT_ID}" "${FAIL_AGENT_ID}" "${LONG_AGENT_ID}"; do
  api -X DELETE "http://127.0.0.1:8080/api/v1/agents/${agent_id}" >/dev/null
done

stop_server
trap - EXIT
cleanup
