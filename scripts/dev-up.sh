#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_DIR="${ROOT_DIR}/.kias-dev"
SECRET_FILE="${STATE_DIR}/jwt-secret"
TOKEN_FILE="${STATE_DIR}/operator-token"

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

require_command docker
require_command curl

if ! docker compose version >/dev/null 2>&1; then
  echo "Docker Compose v2 is required (docker compose)." >&2
  exit 1
fi

mkdir -p "${STATE_DIR}"
chmod 700 "${STATE_DIR}"

if [[ ! -s "${SECRET_FILE}" ]]; then
  if command -v openssl >/dev/null 2>&1; then
    openssl rand -hex 32 >"${SECRET_FILE}"
  elif command -v python3 >/dev/null 2>&1; then
    python3 - <<'PY' >"${SECRET_FILE}"
import secrets
print(secrets.token_hex(32))
PY
  else
    echo "OpenSSL or Python 3 is required to generate a local JWT secret." >&2
    exit 1
  fi
  chmod 600 "${SECRET_FILE}"
fi

export KIAS_JWT_SECRET
KIAS_JWT_SECRET="$(tr -d '\r\n' <"${SECRET_FILE}")"

cd "${ROOT_DIR}"
docker compose up --detach --build

for attempt in $(seq 1 60); do
  if curl --fail --silent http://127.0.0.1:8080/health >/dev/null; then
    break
  fi
  if [[ "${attempt}" -eq 60 ]]; then
    echo "KIAS did not become healthy. Recent logs:" >&2
    docker compose logs --tail=120 kias >&2 || true
    exit 1
  fi
  sleep 2
done

TOKEN="$(docker compose run --rm --no-deps kias token --role operator --subject local-docker-operator | tail -n 1)"
printf '%s\n' "${TOKEN}" >"${TOKEN_FILE}"
chmod 600 "${TOKEN_FILE}"

curl --fail --silent \
  -H "Authorization: Bearer ${TOKEN}" \
  http://127.0.0.1:8080/api/v1/system/capabilities >/dev/null

cat <<EOF
KIAS is running.

Dashboard/API base: http://127.0.0.1:8080
Health:             http://127.0.0.1:8080/health
Operator token:     ${TOKEN_FILE}

Use this token as:
  Authorization: Bearer \$(cat ${TOKEN_FILE})

Stop KIAS with:
  ./scripts/dev-down.sh
EOF
