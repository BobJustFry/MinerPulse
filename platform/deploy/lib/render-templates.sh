#!/usr/bin/env bash
set -euo pipefail

render_file() {
  local template="$1"
  local output="$2"
  cp "$template" "$output"
  while IFS= read -r line; do
    if [[ "$line" =~ ^[A-Z_]+= ]]; then
      key="${line%%=*}"
      val="${line#*=}"
      sed -i "s|{{${key}}}|${val}|g" "$output"
    fi
  done <"$ENV_RENDER_FILE"
}

render_external_proxy_snippet() {
  local root="$1"
  local upstream="${HOST_UPSTREAM:?HOST_UPSTREAM required for external-proxy snippet}"

  mkdir -p "$root/deploy/generated"
  cp "$root/deploy/templates/external-proxy-snippet.txt" "$root/deploy/generated/host-proxy.conf"
  sed -i "s|{{BASE_DOMAIN}}|${BASE_DOMAIN}|g" "$root/deploy/generated/host-proxy.conf"
  sed -i "s|{{WEB_UPSTREAM}}|${upstream}|g" "$root/deploy/generated/host-proxy.conf"
  sed -i "s|{{API_UPSTREAM}}|${upstream}|g" "$root/deploy/generated/host-proxy.conf"
  sed -i "s|{{ADMIN_UPSTREAM}}|${upstream}|g" "$root/deploy/generated/host-proxy.conf"

  {
    echo
    echo "# HOST_UPSTREAM=${upstream} (auto-detected $(date -Iseconds))"
  } >>"$root/deploy/generated/host-proxy.conf"
}

render_templates() {
  local root="$1"
  ENV_RENDER_FILE="$root/deploy/generated/render.env"
  mkdir -p "$root/deploy/generated" "$root/secrets"
  render_file "$root/deploy/templates/env.tpl" "$root/.env"
  render_file "$root/deploy/templates/Caddyfile.tpl" "$root/Caddyfile"
  if [[ -n "${ADMIN_IP_BLOCK:-}" ]]; then
    export ADMIN_IP_BLOCK
    python3 - <<'PY' "$root/Caddyfile"
import os, sys
path = sys.argv[1]
block = os.environ["ADMIN_IP_BLOCK"]
text = open(path, encoding="utf-8").read()
text = text.replace("{{ADMIN_IP_BLOCK}}", block)
open(path, "w", encoding="utf-8").write(text)
PY
  fi
  if [[ -n "${HTTP_TO_HTTPS_REDIRECT:-}" ]]; then
    export HTTP_TO_HTTPS_REDIRECT
    python3 - <<'PY' "$root/Caddyfile"
import os, sys
path = sys.argv[1]
block = os.environ["HTTP_TO_HTTPS_REDIRECT"]
text = open(path, encoding="utf-8").read()
text = text.replace("{{HTTP_TO_HTTPS_REDIRECT}}", block)
open(path, "w", encoding="utf-8").write(text)
PY
  else
    sed -i 's|{{HTTP_TO_HTTPS_REDIRECT}}||g' "$root/Caddyfile" 2>/dev/null || \
      python3 - <<'PY' "$root/Caddyfile"
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace("{{HTTP_TO_HTTPS_REDIRECT}}", "")
open(path, "w", encoding="utf-8").write(text)
PY
  fi
}
