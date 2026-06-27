# Security

## Reporting

Open a private [GitHub Security Advisory](https://github.com/BobJustFry/MinerPulse/security/advisories/new) or contact the owner via Telegram [@miner_pulse](https://t.me/miner_pulse).

## Repository audit (last review: 2026-06-27)

| Item | Status |
|------|--------|
| Private signing key `.tauri/minerpulse.key` | ✅ Not in git (gitignored); only in GitHub Actions **Secrets** |
| Public updater pubkey in `tauri.conf.json` | ✅ OK — must be public for signature verification |
| `releases/update.json` signature | ✅ OK — minisign signature of installer, not a secret |
| `.env` / credentials files | ✅ Not tracked |
| `Documents/` (vendor API PDFs) | ✅ Untracked; added to `.gitignore` |
| `OldProject/` (local prototypes) | ✅ Not tracked; added to `.gitignore` |
| Git history scan for private keys / PATs | ✅ No matches found |
| Donation wallet in README / About | ℹ️ Intentionally public |
| Developer name in About / LICENSE | ℹ️ Intentionally public |

**GitHub Actions secrets (expected):** `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — values are **not** in the repository.

## What must never be committed

- `.tauri/*.key` (Tauri updater **private** signing key)
- `.env`, API keys, JWT signing private keys, Stripe/Paddle keys
- TLS certificates / `.pem` / `.p12` with private material
- Database passwords, production URLs with embedded tokens
- User miner passwords (only stored locally in app `localStorage`, never sent to GitHub)

Before push, run:

```bash
node scripts/check-secrets.mjs --all
```

CI runs the same check on every push (`.github/workflows/secrets.yml`).

## Update integrity

- Updates are distributed via **signed** installers (minisign / Tauri updater).
- Manifest: [releases/update.json](releases/update.json)
- Public key: `minerpulse-desktop/src-tauri/tauri.conf.json` → `plugins.updater.pubkey`
- Private key: local `.tauri/minerpulse.key` + GitHub Actions secret `TAURI_SIGNING_PRIVATE_KEY`

Never disable updater signature verification in the client.

## Subscription / licensing (client vs server)

- Release builds start on **Free** tier; dev tier cycling is **disabled** in production.
- `set_tier` is a **debug-only** helper — not a license system.
- Future: entitlements and API keys must be validated on **minerpulse-api** (private), not only in the desktop app.

## Future: keep encryption & API keys private

Planned split (see [REPOSITORY.md](REPOSITORY.md)):

| Component | Visibility |
|-----------|------------|
| `minerpulse-desktop` + UI shell | Public (this repo) |
| `minerpulse-core` drivers (current) | Public for now |
| **License JWT verification, API keys, billing** | **Private** repo / server only |
| **Proprietary encryption / entitlement algorithms** | **Private** crate or server-side |

Do not add production API keys or encryption secrets to this public tree. Use `.env` (gitignored) locally and GitHub **Secrets** in CI. Template: [.env.example](../.env.example).

## Installer permissions (Windows)

- NSIS `installMode: perMachine` — installs for all users under Program Files; **requires administrator** at install time (UAC).
- Runtime: no admin required for LAN miner TCP (local network).
- Updater: downloads and installs signed package in-app (passive mode on Windows).

## Local app data

- Miner IP/port and optional WhatsMiner LuCI credentials are stored in **browser localStorage** on the user's machine (plain text). They are **not** uploaded to GitHub or Miner Pulse servers unless you add such a feature explicitly.

## WhatsMiner default credentials in source

`fetch_options.rs` includes factory defaults (`root`/`root`, `admin`/`admin`) used to try LuCI on miners. These are **device factory defaults**, not your personal passwords.
