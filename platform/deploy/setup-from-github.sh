#!/usr/bin/env bash
# WSL/Git Bash wrapper — same as setup-from-github.ps1

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONFIG="${1:-$ROOT_DIR/deploy/deploy.config}"
ACTION="${2:-install}"

if [[ ! -f "$CONFIG" ]]; then
  echo "deploy.config not found — run: bash deploy/configure.sh" >&2
  exit 1
fi

# shellcheck source=deploy/lib/load-config.sh
source "$ROOT_DIR/deploy/lib/load-config.sh"
load_deploy_config "$CONFIG"

: "${VPS_HOST:?VPS_HOST required}"
: "${GITHUB_REPO:?GITHUB_REPO required}"

VPS_USER="${VPS_USER:-root}"
SSH_KEY="${SSH_KEY:-}"
TARGET="${VPS_USER}@${VPS_HOST}"
REMOTE_BOOT="/tmp/minerpulse-bootstrap"
TAR="$(mktemp /tmp/minerpulse-bootstrap.XXXXXX.tgz)"
trap 'rm -f "$TAR"' EXIT

SSH_OPTS=(-o BatchMode=yes)
SCP_OPTS=()
if [[ -n "$SSH_KEY" ]]; then
  SSH_OPTS+=(-i "$SSH_KEY")
  SCP_OPTS+=(-i "$SSH_KEY")
fi

echo "=== MinerPulse GitHub deploy -> ${TARGET} (${ACTION}) ==="

tar -czf "$TAR" -C "$ROOT_DIR/deploy" bootstrap-from-github.sh lib templates
ssh "${SSH_OPTS[@]}" "$TARGET" "rm -rf '$REMOTE_BOOT' && mkdir -p '$REMOTE_BOOT'"
scp "${SCP_OPTS[@]}" "$TAR" "${TARGET}:${REMOTE_BOOT}/bootstrap.tgz"
scp "${SCP_OPTS[@]}" "$CONFIG" "${TARGET}:${REMOTE_BOOT}/deploy.config"

ssh "${SSH_OPTS[@]}" "$TARGET" "set -e
cd '$REMOTE_BOOT'
tar -xzf bootstrap.tgz
rm -f bootstrap.tgz
chmod +x bootstrap-from-github.sh lib/*.sh
bash bootstrap-from-github.sh '$REMOTE_BOOT/deploy.config' '$ACTION'"

echo "=== Done ==="
