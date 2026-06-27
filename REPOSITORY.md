# Repository structure and source protection

## Licensing (read first)

- **[LICENSE](LICENSE)** — legal terms (English)
- **[LICENSING.md](LICENSING.md)** — plain-language summary (Russian)

**Forking, copying, and plagiarism are prohibited without written permission from the owner.**

## What is public

This repository contains the **Miner Pulse client** (Tauri UI, Rust drivers, update manifest, documentation). It is public for transparency, issue tracking, and signed updates.

**Public source ≠ open source.** No license is granted except as stated in [LICENSE](LICENSE).

## Forking on GitHub

GitHub may show a **Fork** button on public repositories. That is a **platform feature**, not permission from the copyright holder.

| What GitHub allows technically | What the license allows legally |
|--------------------------------|----------------------------------|
| Anyone can click Fork | Only with **written permission** from the owner |
| Fork copies git history | Fork **does not** grant use, redistribution, or rebranding rights |

Unauthorized forks, mirrors, republished code, or derivative products may be reported and removed (DMCA and applicable law).

## Protection strategy

| Layer | Measure |
|-------|---------|
| Legal | [LICENSE](LICENSE) + [LICENSING.md](LICENSING.md) — proprietary, all rights reserved |
| Metadata | `package.json` / manifests marked proprietary — not MIT/GPL |
| Business logic | Subscription entitlements validated **only on server** (JWT) |
| Updates | Signed packages only (`tauri-plugin-updater` + private signing key) |
| Secrets | Never commit `.tauri/*.key`, API keys, Stripe secrets |
| Optional split | Move sensitive drivers to a **private** repo/submodule later |
| Releases | Users install **signed binaries** from GitHub Releases, not raw git clone |

## Branches

| Branch | Purpose |
|--------|---------|
| `main` | Stable source + `releases/update.json` for production updates |
| `develop` | Integration (optional) |
| Tags | `v1.0.0-build{N}` — one tag per release build |

## Not in this repo

- Private signing key — `.tauri/minerpulse.key` (gitignored)
- `minerpulse-api` / `minerpulse-admin` — planned; may live in separate repos

## Future: private components (encryption & API keys)

The following must **not** live in the public GitHub tree when implemented:

| Secret / logic | Where it belongs |
|----------------|------------------|
| Production API keys | Server env / GitHub Actions Secrets |
| JWT signing **private** key | `minerpulse-api` (private) only |
| License / entitlement crypto | Private crate or server-side validation |
| Stripe / payment webhooks | Private backend only |
| Customer database credentials | Private infra only |

Public repo may contain:

- Client UI and open driver scaffolding
- **Public** JWT verify key (when added) — never the signing private key
- Updater **public** pubkey (already in `tauri.conf.json`)

Local development: copy [.env.example](.env.example) → `.env` (gitignored).

Run `node scripts/check-secrets.mjs --all` before every push.

## If you need stronger technical protection

1. Make the repo **private** and publish **releases only** (binaries + update.json via public raw URL).
2. Keep `minerpulse-core` proprietary drivers in a **private submodule**.
3. Deliver critical parsing via **server-side** hash-map and entitlement APIs only.

For fork or commercial licensing, contact the owner via [LICENSING.md](LICENSING.md).
