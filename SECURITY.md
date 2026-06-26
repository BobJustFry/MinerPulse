# Security

## Reporting

Open a private GitHub Security Advisory or contact the repository owner.

## Update integrity

- Updates are distributed via signed installers (minisign / Tauri updater).
- Manifest: [releases/update.json](releases/update.json)
- Public key: `minerpulse-desktop/src-tauri/tauri.conf.json` → `plugins.updater.pubkey`
- Private key: stored locally in `.tauri/minerpulse.key` and GitHub Actions secret `TAURI_SIGNING_PRIVATE_KEY`

## Installer permissions (Windows)

- NSIS `installMode: perMachine` — installs for all users under Program Files; **requires administrator** at install time (UAC).
- Runtime: no admin required for LAN miner TCP (local network).
- Updater: downloads and installs signed package in-app (passive mode on Windows).

## Secrets checklist (never commit)

- `.tauri/minerpulse.key`
- Stripe / Paddle keys
- JWT signing private keys for production API
- Database credentials
