# Miner Pulse Platform

Subscription backend for the MinerPulse desktop app. Lives in the **MinerPulse monorepo** under `platform/`.

- **Web:** `https://mpulse.bob4.fun`
- **API:** `https://api.mpulse.bob4.fun`
- **Admin:** `https://admin.mpulse.bob4.fun`

## Structure

```
MinerPulse/
  minerpulse-desktop/   # Tauri app
  minerpulse-core/      # Rust core
  platform/             # this folder — API, web, admin, deploy
    apps/api/
    apps/web/
    apps/admin/
    packages/db/
    deploy/
    docker-compose.yml
```

On VPS the whole repo is cloned (e.g. `/opt/minerpulse`), Docker runs from `platform/`.

## Deploy from GitHub

### 1. Configure

```powershell
cd P:\Projects\MinerPulse\platform
copy deploy\deploy.config.example deploy\deploy.config
notepad deploy\deploy.config
```

Set `GITHUB_REPO` to your **MinerPulse** repo, `GITHUB_TOKEN` (private), `VPS_HOST`. Super admin defaults to `mpulse-admin` (password auto-generated).

For SharedAI on 80/443:

- `MPULSE_DEPLOY_MODE=external-proxy`
- `AUTO_INTEGRATE_PROXY=1` + `SHARED_AI_DIR=/opt/sharedai` (optional)

### 2. One-command setup on VPS

```powershell
powershell -ExecutionPolicy Bypass -File deploy\setup-from-github.ps1
```

Clones/pulls MinerPulse → runs `platform/deploy/install.sh` → Docker stack in `/opt/minerpulse/platform`.

Update:

```powershell
powershell -ExecutionPolicy Bypass -File deploy\setup-from-github.ps1 -Action update
```

Credentials: `/opt/minerpulse/platform/deploy/generated/credentials.txt`

## Deploy modes

| Mode | When |
|------|------|
| `standalone` | Free ports 80 and 443 |
| `custom-ports` | 443 busy — HTTPS on **8443**, HTTP on **80** |
| `external-proxy` | Existing reverse proxy on 80/443 (SharedAI) |

## Local dev

```bash
cd platform
cp .env.example .env
npm install
docker compose up postgres -d
npm run db:generate
npm run db:migrate -w @minerpulse/db
npm run db:seed -w @minerpulse/db
npm run dev:api
```

## Stack

- PostgreSQL + Prisma
- Hono API (JWT RS256 licenses)
- Static web + admin (nginx in Docker)
- Caddy reverse proxy (standalone profile)
