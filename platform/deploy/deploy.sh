#!/usr/bin/env bash
# Wrapper for deploy/deploy.ps1 (Windows) or direct SSH deploy from WSL/Git Bash.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONFIG="${1:-$ROOT_DIR/deploy/deploy.config}"

if [[ ! -f "$CONFIG" ]]; then
  echo "Copy deploy/deploy.config.example -> deploy/deploy.config and edit it." >&2
  exit 1
fi

# shellcheck source=deploy/lib/load-config.sh
source "$ROOT_DIR/deploy/lib/load-config.sh"
load_deploy_config "$CONFIG"

: "${VPS_HOST:?VPS_HOST required in deploy.config}"
VPS_USER="${VPS_USER:-root}"
REMOTE_DIR="${REMOTE_DIR:-/opt/mpulse}"
SSH_KEY="${SSH_KEY:-}"

SSH_OPTS=(-o BatchMode=yes)
SCP_OPTS=()
if [[ -n "$SSH_KEY" ]]; then
  SSH_OPTS+=(-i "$SSH_KEY")
  SCP_OPTS+=(-i "$SSH_KEY")
fi
TARGET="${VPS_USER}@${VPS_HOST}"

echo "=== MinerPulse deploy -> ${TARGET}:${REMOTE_DIR} ==="

TAR="$(mktemp /tmp/minerpulse-platform.XXXXXX.tgz)"
trap 'rm -f "$TAR"' EXIT

echo "[1/4] Packing..."
tar -czf "$TAR" \
  --exclude=node_modules \
  --exclude=.env \
  --exclude=secrets \
  --exclude=deploy/generated \
  --exclude=deploy/deploy.config \
  --exclude=.git \
  -C "$ROOT_DIR" .

echo "[2/4] Uploading..."
ssh "${SSH_OPTS[@]}" "$TARGET" "mkdir -p '$REMOTE_DIR'"
scp "${SCP_OPTS[@]}" "$TAR" "${TARGET}:${REMOTE_DIR}/minerpulse-platform.tgz"
ssh "${SSH_OPTS[@]}" "$TARGET" "set -e; cd '$REMOTE_DIR'; tar -xzf minerpulse-platform.tgz; rm -f minerpulse-platform.tgz; chmod +x deploy/*.sh deploy/lib/*.sh 2>/dev/null || true"

echo "[3/4] Installing..."
ssh "${SSH_OPTS[@]}" "$TARGET" "set -e
cd '$REMOTE_DIR'
export MPULSE_NONINTERACTIVE=1
export MPULSE_BASE_DOMAIN='${MPULSE_BASE_DOMAIN:-mpulse.bob4.fun}'
export MPULSE_LETSENCRYPT_EMAIL='${MPULSE_LETSENCRYPT_EMAIL:-admin@bob4.fun}'
export MPULSE_DEPLOY_MODE='${MPULSE_DEPLOY_MODE:-external-proxy}'
export MPULSE_ADMIN_IP_ALLOWLIST='${MPULSE_ADMIN_IP_ALLOWLIST:-}'
export MPULSE_LICENSE_OFFLINE_GRACE_DAYS='${MPULSE_LICENSE_OFFLINE_GRACE_DAYS:-14}'
export MPULSE_BOOTSTRAP_ADMIN_USERNAME='${MPULSE_BOOTSTRAP_ADMIN_USERNAME:-mpulse-admin}'
export MPULSE_BOOTSTRAP_ADMIN_PASSWORD='${MPULSE_BOOTSTRAP_ADMIN_PASSWORD:-}'
export MPULSE_POSTGRES_PASSWORD='${MPULSE_POSTGRES_PASSWORD:-}'
bash deploy/install.sh"

if [[ "${AUTO_INTEGRATE_PROXY:-0}" == "1" ]]; then
  echo "[4/4] Integrating external proxy..."
  ssh "${SSH_OPTS[@]}" "$TARGET" "set -e
cd '$REMOTE_DIR'
export SHARED_AI_DIR='${SHARED_AI_DIR:-/opt/sharedai}'
export SHARED_AI_CADDYFILE='${SHARED_AI_CADDYFILE:-}'
export SHARED_AI_COMPOSE_FILE='${SHARED_AI_COMPOSE_FILE:-}'
export SHARED_AI_CADDY_SERVICE='${SHARED_AI_CADDY_SERVICE:-caddy}'
bash deploy/integrate-external-proxy.sh"
else
  echo "[4/4] Proxy integration skipped (AUTO_INTEGRATE_PROXY=0)"
fi

echo
echo "=== Done ==="
echo "Credentials: ssh $TARGET cat ${REMOTE_DIR}/deploy/generated/credentials.txt"
