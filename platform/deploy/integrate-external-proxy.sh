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

find_caddy_container() {
  local name
  name="$(docker ps --filter "publish=443" --format '{{.Names}}' | head -1)"
  if [[ -n "$name" ]]; then
    echo "$name"
    return 0
  fi
  docker ps --filter "ancestor=caddy" --format '{{.Names}}' | head -1
}

apply_snippet_to_caddyfile() {
  local caddyfile="$1"
  local snippet="$2"
  local marker="$3"
  python3 - "$caddyfile" "$snippet" "$marker" <<'PY'
import sys

caddyfile_path, snippet_path, marker = sys.argv[1:4]
snippet_lines = [ln for ln in open(snippet_path, encoding="utf-8").read().splitlines()
                 if ln.strip() and not ln.strip().startswith("# HOST_UPSTREAM=")]
snippet = "\n".join(snippet_lines).strip() + "\n"

def site_headers(text):
    headers = []
    for line in text.splitlines():
        s = line.strip()
        if not s or s.startswith("#"):
            continue
        if s.endswith("{"):
            headers.append(s[:-1].strip())
    return headers

remove_headers = set(site_headers(snippet))

def strip_blocks(text):
    lines = text.splitlines(keepends=True)
    out = []
    i = 0
    while i < len(lines):
        raw = lines[i]
        stripped = raw.strip()
        header = stripped[:-1].strip() if stripped.endswith("{") else ""
        if header in remove_headers:
            depth = stripped.count("{") - stripped.count("}")
            i += 1
            while i < len(lines) and depth > 0:
                depth += lines[i].count("{") - lines[i].count("}")
                i += 1
            continue
        out.append(raw)
        i += 1
    return "".join(out).rstrip() + "\n"

text = open(caddyfile_path, encoding="utf-8").read()
if marker in text:
    text = text.split(marker, 1)[0]

text = strip_blocks(text)
while text.endswith("\n\n"):
    text = text[:-1]
text = text.rstrip() + "\n\n" + marker + "\n" + snippet

open(caddyfile_path, "w", encoding="utf-8").write(text)
PY
}

reload_caddy() {
  local compose_file="$1"
  local cid

  cid="$(find_caddy_container)"
  if [[ -n "$cid" ]]; then
    echo "[*] Restarting Caddy container ${cid} (TLS + config)..."
    docker restart "$cid" >/dev/null
    return 0
  fi

  if [[ -n "$compose_file" && -f "$compose_file" ]]; then
    (cd "$(dirname "$compose_file")" && docker compose -f "$(basename "$compose_file")" restart "$SHARED_AI_CADDY_SERVICE") && return 0
  fi
  if [[ -f "${SHARED_AI_DIR}/docker-compose.yml" ]]; then
    (cd "$SHARED_AI_DIR" && docker compose restart "$SHARED_AI_CADDY_SERVICE") && return 0
  fi

  echo "[!] Could not restart Caddy automatically. Restart your reverse proxy manually." >&2
  return 1
}

CADDYFILE="$(find_caddyfile)" || {
  echo "Caddyfile not found under ${SHARED_AI_DIR} or via Docker." >&2
  echo "Manual fix:" >&2
  echo "  1) cat ${SNIPPET}" >&2
  echo "  2) Append to your host Caddyfile (container on :443)" >&2
  echo "  3) docker restart <caddy-container>" >&2
  exit 1
}

echo "[*] Updating MinerPulse blocks in ${CADDYFILE}"
cp "$CADDYFILE" "${CADDYFILE}.bak.$(date +%Y%m%d%H%M%S)"
apply_snippet_to_caddyfile "$CADDYFILE" "$SNIPPET" "$MARKER"

COMPOSE_FILE="$SHARED_AI_COMPOSE_FILE"
if [[ -z "$COMPOSE_FILE" && -f "${SHARED_AI_DIR}/docker-compose.yml" ]]; then
  COMPOSE_FILE="${SHARED_AI_DIR}/docker-compose.yml"
fi

reload_caddy "$COMPOSE_FILE"
echo "[ok] External proxy integration done."
