# MinerPulse releases

## Update manifest

Desktop app checks **`update.json`** in this folder (via GitHub raw URL):

`https://raw.githubusercontent.com/BobJustFry/MinerPulse/main/releases/update.json`

After each GitHub Release:

1. Upload signed installer (`.exe`) and signature (`.sig`) from `target/release/bundle/nsis/`.
2. Update `update.json`: `build`, `pub_date`, `platforms.windows-x86_64.url`, `signature`, and `version` as **`X.Y.<build>`** (e.g. product `1.0.1` build `42` → `"version": "1.0.42"`) so Tauri updater compares builds, not only `1.0.1`.
3. Ensure `VERSION.json` build was bumped after code changes (`node scripts/bump-build.mjs`).
4. Commit and push `update.json` (and release artifacts metadata as needed).

## Signing

Generate key once (private key **never** commit):

```bash
cd minerpulse-desktop
npm run tauri signer generate -- -w ../../.tauri/minerpulse.key
```

Set `TAURI_SIGNING_PRIVATE_KEY` in GitHub Actions secrets for CI builds.

Public key lives in `minerpulse-desktop/src-tauri/tauri.conf.json` → `plugins.updater.pubkey`.
