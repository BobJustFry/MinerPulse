#!/usr/bin/env bash
# Detect host IP reachable from a Docker-based reverse proxy (Caddy/nginx in container).

upstream_reachable() {
  local ip="$1"
  local code
  code="$(curl -s -o /dev/null -w '%{http_code}' --connect-timeout 2 "http://${ip}:3001/v1/health" 2>/dev/null || echo "000")"
  [[ "$code" == "200" ]]
}

collect_upstream_candidates() {
  local -a candidates=()
  local cid gw docker0_ip net

  if [[ -n "${MPULSE_HOST_UPSTREAM:-}" ]]; then
    printf '%s\n' "$MPULSE_HOST_UPSTREAM"
    return 0
  fi

  cid="$(docker ps --filter "publish=443" --format '{{.ID}}' | head -1)"
  if [[ -z "$cid" ]]; then
    cid="$(docker ps --filter "ancestor=caddy" --format '{{.ID}}' | head -1)"
  fi
  if [[ -n "$cid" ]]; then
    while IFS= read -r gw; do
      [[ -n "$gw" && "$gw" != "<no value>" ]] && candidates+=("$gw")
    done < <(docker inspect "$cid" --format '{{range .NetworkSettings.Networks}}{{.Gateway}}{{println}}{{end}}' 2>/dev/null || true)
  fi

  if ip -4 addr show docker0 >/dev/null 2>&1; then
    docker0_ip="$(ip -4 addr show docker0 | awk '/inet / {print $2}' | cut -d/ -f1 | head -1)"
    [[ -n "$docker0_ip" ]] && candidates+=("$docker0_ip")
  fi

  while IFS= read -r net; do
    [[ -n "$net" ]] && candidates+=("$net")
  done < <(docker network ls --format '{{.Name}}' 2>/dev/null | while read -r net; do
    docker network inspect "$net" --format '{{range .IPAM.Config}}{{.Gateway}}{{println}}{{end}}' 2>/dev/null || true
  done)

  candidates+=(172.17.0.1 172.18.0.1 172.19.0.1 172.20.0.1)

  printf '%s\n' "${candidates[@]}" | awk '!seen[$0]++ && $0 != ""'
}

detect_host_upstream() {
  local ip

  if [[ -n "${MPULSE_HOST_UPSTREAM:-}" ]]; then
    if upstream_reachable "$MPULSE_HOST_UPSTREAM"; then
      echo "$MPULSE_HOST_UPSTREAM"
      return 0
    fi
    echo "[!] MPULSE_HOST_UPSTREAM=${MPULSE_HOST_UPSTREAM} is set but API health check failed on :3001" >&2
    echo "$MPULSE_HOST_UPSTREAM"
    return 1
  fi

  while IFS= read -r ip; do
    [[ -z "$ip" ]] && continue
    if upstream_reachable "$ip"; then
      echo "[*] Detected host upstream for Docker proxy: ${ip}" >&2
      echo "$ip"
      return 0
    fi
  done < <(collect_upstream_candidates)

  echo "[!] Could not verify host upstream; defaulting to 172.17.0.1 (docker0 gateway)" >&2
  echo "[!] Start platform stack first, or set MPULSE_HOST_UPSTREAM in deploy.config" >&2
  echo "172.17.0.1"
  return 1
}
