#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

CONFIG="${ROOT_DIR}/deploy/deploy.config"
if [[ -f "$CONFIG" ]]; then
  # shellcheck source=deploy/lib/load-config.sh
  source "$ROOT_DIR/deploy/lib/load-config.sh"
  load_deploy_config "$CONFIG"
fi

if [[ -f "$ROOT_DIR/deploy/deploy.config" ]]; then
  git pull --ff-only
else
  git pull
fi

docker compose build

profiles=""
if [[ -f .env ]]; then
  deploy_mode="$(grep -E '^DEPLOY_MODE=' .env | cut -d= -f2- || true)"
  if [[ "$deploy_mode" == "standalone" || "$deploy_mode" == "custom-ports" ]]; then
    profiles="--profile standalone"
  fi
fi

# shellcheck disable=SC2086
docker compose $profiles up -d
echo "Update complete."
