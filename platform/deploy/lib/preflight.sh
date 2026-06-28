#!/usr/bin/env bash
set -euo pipefail

check_port() {
  local port="$1"
  if command -v ss >/dev/null 2>&1; then
    ss -tlnp | grep -E ":${port}\b" || true
  elif command -v lsof >/dev/null 2>&1; then
    lsof -iTCP:"$port" -sTCP:LISTEN || true
  fi
}

port_in_use() {
  local port="$1"
  check_port "$port" | grep -q .
}

describe_port() {
  local port="$1"
  echo "[!] Port ${port} is in use:"
  check_port "$port" | sed 's/^/    /'
}

check_docker() {
  if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
    echo "[ok] Docker is available"
    return 0
  fi
  return 1
}

check_compose() {
  docker compose version >/dev/null 2>&1
}

preflight_ports() {
  local http_port="$1"
  local https_port="$2"
  local deploy_mode="$3"
  local conflict=0

  if [[ "$deploy_mode" == "external-proxy" ]]; then
    return 0
  fi

  if port_in_use "$http_port"; then describe_port "$http_port"; conflict=1; fi
  if port_in_use "$https_port"; then describe_port "$https_port"; conflict=1; fi

  if [[ "$conflict" -eq 1 ]]; then
    echo
    echo "Options:"
    echo "  1) Abort — free ports manually, run install.sh again"
    echo "  2) Stop nginx/apache/caddy on host (destructive if other sites run)"
    echo "  3) External proxy mode — bind app to localhost, use existing reverse proxy on 443"
    echo "  4) Custom HTTPS port — keep HTTP on 80 (Let's Encrypt), HTTPS on 8443"
    return 1
  fi
  return 0
}
