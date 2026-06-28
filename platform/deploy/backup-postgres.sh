#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
mkdir -p "$ROOT_DIR/backups"
FILE="$ROOT_DIR/backups/mpulse-$(date +%Y%m%d-%H%M%S).sql"
docker compose exec -T postgres pg_dump -U mpulse mpulse >"$FILE"
echo "Backup: $FILE"
