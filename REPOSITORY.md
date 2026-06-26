# Repository structure and source protection

## What is public

This repository contains the **MinerPulse client shell** (Tauri UI, open Rust driver scaffolding,
update manifest, documentation). It is public for transparency, issue tracking, and signed updates.

## What cannot be blocked on GitHub

**A public repository can always be forked.** GitHub does not provide a setting to disable forks
while keeping the repo public. Forking copies visible history; it does not grant license to use
your trademarks or bypass server-side subscription checks.

## Protection strategy (recommended)

| Layer | Measure |
|-------|---------|
| Legal | [LICENSE](LICENSE) — proprietary, all rights reserved |
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

- `OldProject/` — legacy AvalonMinerViewer reference (local only, gitignored)
- Private signing key — `.tauri/minerpulse.key` (gitignored)
- `minerpulse-api` / `minerpulse-admin` — planned; may live in separate repos

## If you need stronger protection

1. Make the repo **private** and publish **releases only** (binaries + update.json via public CDN/raw URL).
2. Keep `minerpulse-core` proprietary drivers in a **private submodule**.
3. Deliver critical parsing via **server-side** hash-map and entitlement APIs only.
