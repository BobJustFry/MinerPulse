#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SNIPPET="${ROOT_DIR}/deploy/generated/host-proxy.conf"
MARKER="# --- MinerPulse platform (managed by integrate-external-proxy.sh) ---"

SHARED_AI_DIR="${SHARED_AI_DIR:-/opt/sharedai}"
SHARED_AI_CADDYFILE="${SHARED_AI_CADDYFILE:-}"
SHARED_AI_COMPOSE_FILE="${SHARED_AI_COMPOSE_FILE:-}"
SHARED_AI_CADDY_SERVICE="${SHARED_AI_CADDY_SERVICE:-caddy}"

if [[ ! -f "$SNIPPET" ]]; then
  echo "Missing ${SNIPPET}. Run deploy/install.sh first." >&2
  exit 1
fi

find_caddyfile() {
  if [[ -n "$SHARED_AI_CADDYFILE" && -f "$SHARED_AI_CADDYFILE" ]]; then
    echo "$SHARED_AI_CADDYFILE"
    return 0
  fi
  local candidates=(
    "${SHARED_AI_DIR}/Caddyfile"
    "${SHARED_AI_DIR}/caddy/Caddyfile"
    "${SHARED_AI_DIR}/config/Caddyfile"
    "${SHARED_AI_DIR}/reverse-proxy/Caddyfile"
    "/opt/sharedai/infra/caddy/Caddyfile"
    "${SHARED_AI_DIR}/data/caddy/Caddyfile"
    "${SHARED_AI_DIR}/caddy/Caddyfile"
  )
  local path
  for path in "${candidates[@]}"; do
    if [[ -f "$path" ]]; then
      echo "$path"
      return 0
    fi
  done
  find_caddyfile_via_docker && return 0
  return 1
}

find_caddyfile_via_docker() {
  local cid line src dest
  for cid in $(docker ps --filter "publish=443" --format '{{.ID}}'); do
    while IFS= read -r line; do
      [[ -z "$line" ]] && continue
      src="${line%% -> *}"
      dest="${line##* -> }"
      if [[ "$dest" == "/etc/caddy/Caddyfile" && -f "$src" ]]; then
        echo "$src"
        return 0
      fi
    done < <(docker inspect "$cid" --format '{{range .Mounts}}{{.Source}} -> {{.Destination}}{{"\n"}}{{end}}')
  done
  for cid in $(docker ps --filter "ancestor=caddy" --format '{{.ID}}'); do
    while IFS= read -r line; do
      [[ -z "$line" ]] && continue
      src="${line%% -> *}"
      dest="${line##* -> }"
      if [[ "$dest" == "/etc/caddy/Caddyfile" && -f "$src" ]]; then
        echo "$src"
        return 0
      fi
    done < <(docker inspect "$cid" --format '{{range .Mounts}}{{.Source}} -> {{.Destination}}{{"\n"}}{{end}}')
  done
  return 1
}

CADDYFILE="$(find_caddyfile)" || {
  echo "Caddyfile not found under ${SHARED_AI_DIR} or via Docker." >&2
  echo "Manual fix:" >&2
  echo "  1) cat ${SNIPPET}" >&2
  echo "  2) Append to your host Caddyfile (container on :443)" >&2
  echo "  3) docker exec <caddy> caddy reload --config /etc/caddy/Caddyfile" >&2
  echo "Find mount: docker ps ; docker inspect <name> --format '{{range .Mounts}}{{.Source}} -> {{.Destination}}{{println}}{{end}}'" >&2
  exit 1
}

if grep -qF "$MARKER" "$CADDYFILE" 2>/dev/null; then
  echo "[ok] MinerPulse proxy blocks already present in ${CADDYFILE}"
else
  echo "[*] Appending MinerPulse blocks to ${CADDYFILE}"
  cp "$CADDYFILE" "${CADDYFILE}.bak.$(date +%Y%m%d%H%M%S)"
  {
    echo
    echo "$MARKER"
    cat "$SNIPPET"
  } >>"$CADDYFILE"
fi

reload_caddy() {
  local compose_file="$1"
  if [[ -n "$compose_file" && -f "$compose_file" ]]; then
    (cd "$(dirname "$compose_file")" && docker compose -f "$(basename "$compose_file")" exec -T "$SHARED_AI_CADDY_SERVICE" caddy reload --config /etc/caddy/Caddyfile) && return 0
  fi
  if [[ -f "${SHARED_AI_DIR}/docker-compose.yml" ]]; then
    (cd "$SHARED_AI_DIR" && docker compose exec -T "$SHARED_AI_CADDY_SERVICE" caddy reload --config /etc/caddy/Caddyfile) && return 0
  fi
  if [[ -f "${SHARED_AI_DIR}/compose.yml" ]]; then
    (cd "$SHARED_AI_DIR" && docker compose exec -T "$SHARED_AI_CADDY_SERVICE" caddy reload --config /etc/caddy/Caddyfile) && return 0
  fi
  local cid
  cid="$(docker ps --filter "publish=443" --format '{{.Names}}' | head -1)"
  if [[ -n "$cid" ]]; then
    docker exec "$cid" caddy reload --config /etc/caddy/Caddyfile
    return 0
  fi
  echo "[!] Could not reload Caddy automatically. Reload your reverse proxy manually." >&2
  return 1
}

COMPOSE_FILE="$SHARED_AI_COMPOSE_FILE"
if [[ -z "$COMPOSE_FILE" && -f "${SHARED_AI_DIR}/docker-compose.yml" ]]; then
  COMPOSE_FILE="${SHARED_AI_DIR}/docker-compose.yml"
fi

reload_caddy "$COMPOSE_FILE" || true
echo "[ok] External proxy integration done."
