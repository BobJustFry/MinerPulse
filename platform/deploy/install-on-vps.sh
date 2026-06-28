#!/usr/bin/env bash
# Run ON the VPS as root — no SSH from your PC needed.
#
#   curl -fsSL https://raw.githubusercontent.com/BobJustFry/MinerPulse/main/platform/deploy/install-on-vps.sh | sudo bash
#
# Or after git clone:
#   sudo bash platform/deploy/install-on-vps.sh /opt/minerpulse/platform/deploy/deploy.config

set -euo pipefail

CONFIG="${1:-}"
REPO_DIR="${REPO_DIR:-/opt/minerpulse}"
PLATFORM_SUBDIR="${PLATFORM_SUBDIR:-platform}"
GITHUB_REPO="${GITHUB_REPO:-https://github.com/BobJustFry/MinerPulse.git}"
GITHUB_BRANCH="${GITHUB_BRANCH:-main}"

if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
  echo "Run as root: sudo bash $0" >&2
  exit 1
fi

if ! command -v git >/dev/null 2>&1; then
  apt-get update -qq
  apt-get install -y git
fi

if [[ ! -d "$REPO_DIR/.git" ]]; then
  echo "[*] Cloning ${GITHUB_REPO} -> ${REPO_DIR}"
  mkdir -p "$(dirname "$REPO_DIR")"
  git clone -b "$GITHUB_BRANCH" "$GITHUB_REPO" "$REPO_DIR"
else
  echo "[*] Updating ${REPO_DIR}"
  cd "$REPO_DIR"
  git pull --ff-only origin "$GITHUB_BRANCH"
fi

PLATFORM_DIR="${REPO_DIR}/${PLATFORM_SUBDIR}"
BOOTSTRAP="${PLATFORM_DIR}/deploy/bootstrap-from-github.sh"

if [[ ! -f "$BOOTSTRAP" ]]; then
  echo "Not found: $BOOTSTRAP" >&2
  exit 1
fi

if [[ -z "$CONFIG" ]]; then
  CONFIG="${PLATFORM_DIR}/deploy/deploy.config"
  if [[ ! -f "$CONFIG" ]]; then
    echo "Create ${CONFIG} first (copy from deploy.config.example) or pass path as argument." >&2
    exit 1
  fi
fi

exec bash "$BOOTSTRAP" "$CONFIG" install
