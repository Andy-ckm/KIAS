#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SECRET_FILE="${ROOT_DIR}/.kias-dev/jwt-secret"

if [[ ! -s "${SECRET_FILE}" ]]; then
  echo "No local KIAS runtime secret was found; nothing to stop." >&2
  exit 0
fi

export KIAS_JWT_SECRET
KIAS_JWT_SECRET="$(tr -d '\r\n' <"${SECRET_FILE}")"

cd "${ROOT_DIR}"
docker compose down
