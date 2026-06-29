#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=deploy/lib/detect-host-upstream.sh
source "$ROOT_DIR/deploy/lib/detect-host-upstream.sh"

BASE_DOMAIN="${MPULSE_BASE_DOMAIN:-}"
if [[ -z "$BASE_DOMAIN" && -f "$ROOT_DIR/.env" ]]; then
  # shellcheck disable=SC1091
  source "$ROOT_DIR/.env"
  BASE_DOMAIN="${BASE_DOMAIN:-mpulse.bob4.fun}"
fi
BASE_DOMAIN="${BASE_DOMAIN:-mpulse.bob4.fun}"

HOST_UPSTREAM="$(detect_host_upstream 2>/dev/null || echo "172.17.0.1")"

echo "=== External proxy verification ==="
echo "Base domain: ${BASE_DOMAIN}"
echo "Host upstream: ${HOST_UPSTREAM}"

fail=0

check_code() {
  local label="$1"
  local url="$2"
  local expect="${3:-200}"
  local code
  code="$(curl -s -o /dev/null -w '%{http_code}' --connect-timeout 5 "$url" 2>/dev/null || echo "000")"
  if [[ "$code" == "$expect" ]]; then
    echo "[ok] ${label}: HTTP ${code}"
  else
    echo "[!] ${label}: HTTP ${code} (expected ${expect}) — ${url}" >&2
    fail=1
  fi
}

check_code "API (host)" "http://${HOST_UPSTREAM}:3001/v1/health"
check_code "Web (host)" "http://${HOST_UPSTREAM}:3000/"
check_code "Admin (host)" "http://${HOST_UPSTREAM}:3002/"

for host in "$BASE_DOMAIN" "api.${BASE_DOMAIN}" "admin.${BASE_DOMAIN}"; do
  if [[ "$host" == "api.${BASE_DOMAIN}" ]]; then
    check_code "HTTPS ${host}" "https://${host}/v1/health"
  else
    check_code "HTTPS ${host}" "https://${host}/"
  fi
done

if [[ "$fail" -ne 0 ]]; then
  exit 1
fi

echo "[ok] External proxy checks passed"
