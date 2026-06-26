# MinerPulse releases

## Update manifest

Desktop app checks **`update.json`** in this folder (via GitHub raw URL):

`https://raw.githubusercontent.com/BobJustFry/MinerPulse/main/releases/update.json`

After each GitHub Release:

1. Upload signed installer (`.exe`) and signature (`.sig`) from `target/release/bundle/nsis/`.
2. Update `update.json`: `version`, `build`, `pub_date`, `platforms.windows-x86_64.url`, `signature`.
3. Commit and push (bump `VERSION.json` build via `node scripts/bump-build.mjs`).

## Signing

Generate key once (private key **never** commit):

```bash
cd minerpulse-desktop
npm run tauri signer generate -- -w ../../.tauri/minerpulse.key
```

Set `TAURI_SIGNING_PRIVATE_KEY` in GitHub Actions secrets for CI builds.

Public key lives in `minerpulse-desktop/src-tauri/tauri.conf.json` → `plugins.updater.pubkey`.
