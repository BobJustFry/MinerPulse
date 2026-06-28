#!/usr/bin/env bash
# Clone/pull MinerPulse monorepo from GitHub and run platform install on VPS.
#
# Usage (on VPS, from platform/deploy/):
#   sudo bash bootstrap-from-github.sh /path/to/deploy.config
#   sudo bash bootstrap-from-github.sh /path/to/deploy.config update

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=deploy/lib/load-config.sh
source "$SCRIPT_DIR/lib/load-config.sh"
# shellcheck source=deploy/lib/export-install-env.sh
source "$SCRIPT_DIR/lib/export-install-env.sh"

CONFIG_FILE="${1:-${MPULSE_DEPLOY_CONFIG:-}}"
ACTION="${2:-install}"

if [[ -n "$CONFIG_FILE" ]]; then
  if [[ ! -f "$CONFIG_FILE" ]]; then
    echo "Config not found: $CONFIG_FILE" >&2
    exit 1
  fi
  load_deploy_config "$CONFIG_FILE"
fi

: "${GITHUB_REPO:?GITHUB_REPO is required in deploy.config}"
REPO_DIR="${REMOTE_DIR:-/opt/minerpulse}"
PLATFORM_SUBDIR="${PLATFORM_SUBDIR:-platform}"
PLATFORM_DIR="${REPO_DIR}/${PLATFORM_SUBDIR}"
GITHUB_BRANCH="${GITHUB_BRANCH:-main}"

if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
  echo "Run as root: sudo bash $0 deploy.config" >&2
  exit 1
fi

ensure_git() {
  if command -v git >/dev/null 2>&1; then
    return 0
  fi
  echo "[*] Installing git..."
  export DEBIAN_FRONTEND=noninteractive
  apt-get update -qq
  apt-get install -y git
}

ensure_docker() {
  if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
    return 0
  fi
  bash "$SCRIPT_DIR/lib/docker-install.sh"
}

github_clone_url() {
  local repo="$GITHUB_REPO"
  if [[ -n "${GITHUB_TOKEN:-}" ]]; then
    if [[ "$repo" =~ ^https://github.com/(.+)$ ]]; then
      repo="https://x-access-token:${GITHUB_TOKEN}@github.com/${BASH_REMATCH[1]}"
    elif [[ "$repo" =~ ^git@github.com:(.+)$ ]]; then
      repo="https://x-access-token:${GITHUB_TOKEN}@github.com/${BASH_REMATCH[1]}"
    fi
  fi
  printf '%s' "$repo"
}

sync_repo() {
  local url branch
  url="$(github_clone_url)"
  branch="$GITHUB_BRANCH"

  if [[ -d "$REPO_DIR/.git" ]]; then
    echo "[*] Updating ${REPO_DIR} (branch ${branch})..."
    cd "$REPO_DIR"
    git remote set-url origin "$url"
    git fetch origin "$branch"
    git checkout "$branch"
    git pull --ff-only origin "$branch"
  else
    echo "[*] Cloning ${GITHUB_REPO} -> ${REPO_DIR}..."
    mkdir -p "$(dirname "$REPO_DIR")"
    git clone -b "$branch" "$url" "$REPO_DIR"
  fi

  if [[ ! -d "$PLATFORM_DIR/deploy" ]]; then
    echo "Platform dir not found: ${PLATFORM_DIR}/deploy" >&2
    echo "Check PLATFORM_SUBDIR=${PLATFORM_SUBDIR} in deploy.config" >&2
    exit 1
  fi

  chmod +x "$PLATFORM_DIR"/deploy/*.sh "$PLATFORM_DIR"/deploy/lib/*.sh 2>/dev/null || true
}

persist_config() {
  if [[ -n "$CONFIG_FILE" && -f "$CONFIG_FILE" ]]; then
    mkdir -p "$PLATFORM_DIR/deploy"
    local dest="$PLATFORM_DIR/deploy/deploy.config"
    if [[ "$CONFIG_FILE" != "$dest" ]]; then
      cp "$CONFIG_FILE" "$dest"
    fi
    chmod 600 "$dest"
  fi
}

run_install() {
  echo "[*] Running install in ${PLATFORM_DIR}..."
  cd "$PLATFORM_DIR"
  export_install_env
  bash deploy/install.sh
}

run_update() {
  echo "[*] Rebuilding containers in ${PLATFORM_DIR}..."
  cd "$PLATFORM_DIR"
  docker compose build
  local profiles=""
  if [[ -f .env ]]; then
    # shellcheck disable=SC1091
    source .env
    if [[ "${DEPLOY_MODE:-}" == "standalone" || "${DEPLOY_MODE:-}" == "custom-ports" ]]; then
      profiles="--profile standalone"
    fi
  fi
  # shellcheck disable=SC2086
  docker compose $profiles up -d
}

integrate_proxy() {
  if [[ "${AUTO_INTEGRATE_PROXY:-0}" != "1" ]]; then
    return 0
  fi
  echo "[*] Integrating external proxy..."
  cd "$PLATFORM_DIR"
  export SHARED_AI_DIR="${SHARED_AI_DIR:-/opt/sharedai}"
  export SHARED_AI_CADDYFILE="${SHARED_AI_CADDYFILE:-}"
  export SHARED_AI_COMPOSE_FILE="${SHARED_AI_COMPOSE_FILE:-}"
  export SHARED_AI_CADDY_SERVICE="${SHARED_AI_CADDY_SERVICE:-caddy}"
  bash deploy/integrate-external-proxy.sh
}

echo "=== MinerPulse platform bootstrap from GitHub ==="
echo "Repo: ${REPO_DIR}  Platform: ${PLATFORM_DIR}"
ensure_git
ensure_docker
sync_repo
persist_config

case "$ACTION" in
  install)
    run_install
    integrate_proxy
    ;;
  update)
    run_update
    ;;
  *)
    echo "Unknown action: $ACTION (use: install | update)" >&2
    exit 1
    ;;
esac

echo
echo "=== Bootstrap complete ==="
if [[ -f "$PLATFORM_DIR/deploy/generated/credentials.txt" ]]; then
  echo "Credentials: ${PLATFORM_DIR}/deploy/generated/credentials.txt"
fi
echo "Update later: sudo bash ${PLATFORM_DIR}/deploy/bootstrap-from-github.sh ${PLATFORM_DIR}/deploy/deploy.config update"
