#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# shellcheck source=deploy/lib/prompt.sh
source "$ROOT_DIR/deploy/lib/prompt.sh"
# shellcheck source=deploy/lib/preflight.sh
source "$ROOT_DIR/deploy/lib/preflight.sh"
# shellcheck source=deploy/lib/render-templates.sh
source "$ROOT_DIR/deploy/lib/render-templates.sh"

if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
  echo "Run as root: sudo bash deploy/install.sh"
  exit 1
fi

echo "=== Miner Pulse Platform installer ==="

if [[ "${MPULSE_NONINTERACTIVE:-}" == "1" ]]; then
  BASE_DOMAIN="${MPULSE_BASE_DOMAIN:-mpulse.bob4.fun}"
  LETSENCRYPT_EMAIL="${MPULSE_LETSENCRYPT_EMAIL:-admin@bob4.fun}"
  DEPLOY_MODE="${MPULSE_DEPLOY_MODE:-external-proxy}"
  HTTP_PORT="${MPULSE_HTTP_PORT:-80}"
  HTTPS_PORT="${MPULSE_HTTPS_PORT:-443}"
  ADMIN_IP_ALLOWLIST="${MPULSE_ADMIN_IP_ALLOWLIST:-}"
  LICENSE_OFFLINE_GRACE_DAYS="${MPULSE_LICENSE_OFFLINE_GRACE_DAYS:-14}"
  BOOTSTRAP_ADMIN_USERNAME="${MPULSE_BOOTSTRAP_ADMIN_USERNAME:-mpulse-admin}"
  BOOTSTRAP_ADMIN_PASSWORD="${MPULSE_BOOTSTRAP_ADMIN_PASSWORD:-}"
  POSTGRES_PASSWORD="${MPULSE_POSTGRES_PASSWORD:-}"
  if [[ -z "$BOOTSTRAP_ADMIN_PASSWORD" ]]; then
    BOOTSTRAP_ADMIN_PASSWORD="$(gen_password)"
  fi
  if [[ -z "$POSTGRES_PASSWORD" ]]; then
    POSTGRES_PASSWORD="$(gen_password)"
  fi
  echo "[*] Non-interactive install: mode=${DEPLOY_MODE}, domain=${BASE_DOMAIN}"
else
  prompt BASE_DOMAIN "Base domain" "mpulse.bob4.fun"
  prompt LETSENCRYPT_EMAIL "Let's Encrypt email" "admin@bob4.fun"
  prompt DEPLOY_MODE "Deploy mode (standalone|external-proxy|custom-ports)" "standalone"

  DEFAULT_HTTP=80
  DEFAULT_HTTPS=443
  if [[ "$DEPLOY_MODE" == "custom-ports" ]]; then
    DEFAULT_HTTP=80
    DEFAULT_HTTPS=8443
  fi

  prompt HTTP_PORT "HTTP port (host -> Caddy :80, keep 80 for Let's Encrypt)" "$DEFAULT_HTTP"
  prompt HTTPS_PORT "HTTPS port (host -> Caddy :443)" "$DEFAULT_HTTPS"
  prompt ADMIN_IP_ALLOWLIST "Admin IP allowlist (CIDR, optional)" ""
  prompt LICENSE_OFFLINE_GRACE_DAYS "Offline grace days" "14"
  prompt BOOTSTRAP_ADMIN_USERNAME "Super admin username" "mpulse-admin"
  prompt_secret BOOTSTRAP_ADMIN_PASSWORD "Super admin password (empty = auto-generate)"

  if [[ -z "$BOOTSTRAP_ADMIN_PASSWORD" ]]; then
    BOOTSTRAP_ADMIN_PASSWORD="$(gen_password)"
  fi
  prompt_secret POSTGRES_PASSWORD "PostgreSQL password (empty = auto)" ""
  if [[ -z "$POSTGRES_PASSWORD" ]]; then
    POSTGRES_PASSWORD="$(gen_password)"
  fi
fi

if ! preflight_ports "$HTTP_PORT" "$HTTPS_PORT" "$DEPLOY_MODE"; then
  if [[ "${MPULSE_NONINTERACTIVE:-}" == "1" ]]; then
    echo "Port preflight failed for ${HTTP_PORT}/${HTTPS_PORT} in mode ${DEPLOY_MODE}." >&2
    echo "Use MPULSE_DEPLOY_MODE=external-proxy when 80/443 are already taken." >&2
    exit 1
  fi
  echo
  read -r -p "Choose option [1-4]: " PORT_CHOICE
  case "$PORT_CHOICE" in
    2)
      confirm "Stop nginx/apache/caddy on host?" && {
        systemctl stop nginx apache2 caddy 2>/dev/null || true
        systemctl disable nginx apache2 caddy 2>/dev/null || true
      } || exit 1
      ;;
    3) DEPLOY_MODE="external-proxy" ;;
    4)
      DEPLOY_MODE="custom-ports"
      HTTP_PORT=80
      HTTPS_PORT=8443
      echo "[*] Using custom-ports: HTTP ${HTTP_PORT}, HTTPS ${HTTPS_PORT}"
      if ! preflight_ports "$HTTP_PORT" "$HTTPS_PORT" "$DEPLOY_MODE"; then
        echo "Ports ${HTTP_PORT}/${HTTPS_PORT} still busy. Free them or choose external-proxy."
        exit 1
      fi
      ;;
    *) echo "Abort."; exit 1 ;;
  esac
fi

if ! check_docker; then
  bash "$ROOT_DIR/deploy/lib/docker-install.sh"
fi

mkdir -p "$ROOT_DIR/secrets"
if [[ ! -f "$ROOT_DIR/secrets/jwt_private.pem" ]]; then
  openssl genrsa -out "$ROOT_DIR/secrets/jwt_private.pem" 2048
  openssl rsa -in "$ROOT_DIR/secrets/jwt_private.pem" -pubout -out "$ROOT_DIR/secrets/jwt_public.pem"
fi

JWT_PRIVATE_KEY="$(one_line_pem "$ROOT_DIR/secrets/jwt_private.pem")"
JWT_PUBLIC_KEY="$(one_line_pem "$ROOT_DIR/secrets/jwt_public.pem")"

PUBLIC_URL_HTTPS_SUFFIX=""
if [[ "$HTTPS_PORT" != "443" ]]; then
  PUBLIC_URL_HTTPS_SUFFIX=":${HTTPS_PORT}"
fi

HTTP_TO_HTTPS_REDIRECT=""
if [[ "$HTTPS_PORT" != "443" ]]; then
  HTTP_TO_HTTPS_REDIRECT=$'  @http protocol http\n  redir @http https://{host}'"${PUBLIC_URL_HTTPS_SUFFIX}"$'{uri} permanent\n'
fi

ADMIN_IP_BLOCK=""
if [[ -n "$ADMIN_IP_ALLOWLIST" ]]; then
  ADMIN_IP_BLOCK=$'  @admin_allowed remote_ip '"${ADMIN_IP_ALLOWLIST}"$'\n  handle @admin_allowed {\n    reverse_proxy admin:3002\n  }\n  respond 403'
else
  ADMIN_IP_BLOCK=$'  reverse_proxy admin:3002'
fi

mkdir -p "$ROOT_DIR/deploy/generated"
cat >"$ROOT_DIR/deploy/generated/render.env" <<EOF
BASE_DOMAIN=${BASE_DOMAIN}
LETSENCRYPT_EMAIL=${LETSENCRYPT_EMAIL}
DEPLOY_MODE=${DEPLOY_MODE}
HTTP_PORT=${HTTP_PORT}
HTTPS_PORT=${HTTPS_PORT}
PUBLIC_URL_HTTPS_SUFFIX=${PUBLIC_URL_HTTPS_SUFFIX}
ADMIN_IP_ALLOWLIST=${ADMIN_IP_ALLOWLIST}
POSTGRES_PASSWORD=${POSTGRES_PASSWORD}
JWT_PRIVATE_KEY=${JWT_PRIVATE_KEY}
JWT_PUBLIC_KEY=${JWT_PUBLIC_KEY}
LICENSE_OFFLINE_GRACE_DAYS=${LICENSE_OFFLINE_GRACE_DAYS}
BOOTSTRAP_ADMIN_USERNAME=${BOOTSTRAP_ADMIN_USERNAME}
BOOTSTRAP_ADMIN_PASSWORD=${BOOTSTRAP_ADMIN_PASSWORD}
EOF

export BASE_DOMAIN DEPLOY_MODE ADMIN_IP_BLOCK HTTP_TO_HTTPS_REDIRECT
render_templates "$ROOT_DIR"

if [[ "$DEPLOY_MODE" == "external-proxy" ]]; then
  cat >"$ROOT_DIR/docker-compose.override.yml" <<'YAML'
services:
  api:
    ports:
      - "127.0.0.1:3001:3001"
  web:
    ports:
      - "127.0.0.1:3000:3000"
  admin:
    ports:
      - "127.0.0.1:3002:3002"
YAML
fi

if command -v ufw >/dev/null 2>&1; then
  ufw allow 22/tcp || true
  if [[ "$DEPLOY_MODE" == "standalone" || "$DEPLOY_MODE" == "custom-ports" ]]; then
    ufw allow "${HTTP_PORT}/tcp" || true
    ufw allow "${HTTPS_PORT}/tcp" || true
  fi
fi

echo "[*] Building containers..."
docker compose build

COMPOSE_PROFILES=""
if [[ "$DEPLOY_MODE" == "standalone" || "$DEPLOY_MODE" == "custom-ports" ]]; then
  COMPOSE_PROFILES="--profile standalone"
fi

echo "[*] Starting stack..."
# shellcheck disable=SC2086
docker compose $COMPOSE_PROFILES up -d

echo "[*] Waiting for API health..."
for i in $(seq 1 30); do
  if docker compose exec -T api node -e "fetch('http://127.0.0.1:3001/v1/health').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))" >/dev/null 2>&1; then
    break
  fi
  sleep 2
done

ADMIN_URL="https://admin.${BASE_DOMAIN}${PUBLIC_URL_HTTPS_SUFFIX}"

CREDS_FILE="$ROOT_DIR/deploy/generated/credentials.txt"
mkdir -p "$(dirname "$CREDS_FILE")"
cat >"$CREDS_FILE" <<EOF
=== Miner Pulse Platform ===
Web:    https://${BASE_DOMAIN}${PUBLIC_URL_HTTPS_SUFFIX}
API:    https://api.${BASE_DOMAIN}${PUBLIC_URL_HTTPS_SUFFIX}
Admin:  ${ADMIN_URL}

=== Super admin (full access) ===
URL:      ${ADMIN_URL}
Username: ${BOOTSTRAP_ADMIN_USERNAME}
Password: ${BOOTSTRAP_ADMIN_PASSWORD}

Deploy mode: ${DEPLOY_MODE}
Postgres password: ${POSTGRES_PASSWORD}
JWT public key: ${ROOT_DIR}/secrets/jwt_public.pem
EOF
chmod 600 "$CREDS_FILE"

cat <<EOF

=== Miner Pulse Platform ready ===

Web:   https://${BASE_DOMAIN}${PUBLIC_URL_HTTPS_SUFFIX}
API:   https://api.${BASE_DOMAIN}${PUBLIC_URL_HTTPS_SUFFIX}
Admin: ${ADMIN_URL}

=== Super admin login ===
  URL:      ${ADMIN_URL}
  Username: ${BOOTSTRAP_ADMIN_USERNAME}
  Password: ${BOOTSTRAP_ADMIN_PASSWORD}

(Password also saved in .env and deploy/generated/credentials.txt)

Deploy mode: ${DEPLOY_MODE}
Postgres pass:  ${POSTGRES_PASSWORD}

JWT public key: ${ROOT_DIR}/secrets/jwt_public.pem

DNS A records (@, api, admin) -> $(curl -4 -s ifconfig.me || echo YOUR_VPS_IP)
Update: sudo bash deploy/update.sh
EOF

if [[ "$DEPLOY_MODE" == "external-proxy" ]]; then
  echo "External proxy snippet: deploy/generated/host-proxy.conf"
  echo "Credentials saved: deploy/generated/credentials.txt"
fi
