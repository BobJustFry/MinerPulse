#!/usr/bin/env bash

mpulse_dns_ready() {
  local base="$1"
  local host missing=0

  for host in "$base" "api.${base}" "admin.${base}"; do
    if ! dig +short A "$host" 2>/dev/null | grep -qE '^[0-9]+\.'; then
      echo "[!] DNS A record missing for ${host}" >&2
      missing=1
    fi
  done

  [[ "$missing" -eq 0 ]]
}

wait_mpulse_dns() {
  local base="$1"
  local max_wait="${2:-120}"
  local elapsed=0

  if mpulse_dns_ready "$base"; then
    echo "[ok] DNS ready for ${base}, api.${base}, admin.${base}"
    return 0
  fi

  echo "[*] Waiting up to ${max_wait}s for DNS A records (@, api, admin) -> VPS..." >&2
  while [[ "$elapsed" -lt "$max_wait" ]]; do
    sleep 10
    elapsed=$((elapsed + 10))
    if mpulse_dns_ready "$base"; then
      echo "[ok] DNS ready after ${elapsed}s"
      return 0
    fi
  done

  echo "[!] DNS still incomplete — TLS certificates may fail until records propagate" >&2
  return 1
}
