# MinerPulse

Multi-vendor ASIC miner monitor — **Tauri 2 + Rust + Svelte**.

**Version:** see [VERSION.json](VERSION.json) (product version + build number).

## Quick start

**Windows (recommended):**

```powershell
cd P:\Projects\AMW\minerpulse-desktop
npm install
npm run dev:app
```

`dev:app` adds Rust/cargo to PATH automatically (fixes `program not found`).

**Other shells:**

```bash
cd minerpulse-desktop
npm install
npm run tauri dev
```

Requires **Rust** in PATH (`winget install Rustlang.Rustup`, then new terminal).

## Versioning

| Field | File | Rule |
|-------|------|------|
| Version `1.0.0` | `VERSION.json` | Change **only with owner approval** |
| Build `N` | `VERSION.json` | Increment before **every commit**: `node scripts/bump-build.mjs` |

## Updates

App checks [releases/update.json](releases/update.json) (GitHub raw URL).  
In-app: toolbar **Update** → download & install signed package.

## Repository policy

- [LICENSE](LICENSE) — proprietary
- [REPOSITORY.md](REPOSITORY.md) — public repo vs IP protection
- [SECURITY.md](SECURITY.md) — signing keys, installer admin rights
- [.cursor/rules/minerpulse-strict.mdc](.cursor/rules/minerpulse-strict.mdc) — strict dev rules for AI/humans

## Structure

```
minerpulse-core/       Rust library
minerpulse-desktop/    Tauri app
releases/              update.json manifest
scripts/               version sync / build bump
assets/matrix/         bundled hash-board templates
```

## GitHub

Public repo: `BobJustFry/MinerPulse`
