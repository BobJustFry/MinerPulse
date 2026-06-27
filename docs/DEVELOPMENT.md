# Development rules

Internal guidelines for Miner Pulse contributors.

## Versioning

- **Product version** (`VERSION.json` → `version`): change **only with owner approval**.
- **Build number** (`VERSION.json` → `build`): increment after **each logical code change** via `node scripts/bump-build.mjs` — independent of commit/push.
- After bumping, `sync-version.mjs` runs automatically (via bump script).
- When you commit, include `build N` in the commit message.
- Tag releases as `v{version}-build{build}`.

## Git workflow

- Bump build after each logical change. Commit and push only when the owner asks or per release workflow.
- Never force-push `main`.
- Never commit secrets (`.tauri/*.key`, `.env`, credentials). Public updater pubkey (`.tauri/*.key.pub`) may be committed when intentionally rotated with config.
- Never commit paths in `.gitignore` (`target/`, `node_modules/`, build artifacts, local-only dirs).
- Run `node scripts/check-secrets.mjs --all` before push.

## Code quality

- Minimize scope; no unrelated refactors.
- Rust: business rules in `minerpulse-core`; UI in Svelte; no user-facing strings in Rust (use `ErrorCode` only).
- All UI strings via i18n (`ru`, `en`, `zh-CN`) — no hardcoded text in components.
- Match existing Design System tokens; do not introduce new UI libraries without approval.

## Security and licensing

- Subscription tier must remain enforceable server-side — do not weaken checks in release builds.
- Do not expose dev-only tier bypass in production.
- Never remove or bypass updater signature verification.
- Never add production API keys or proprietary encryption to the public tree — see [SECURITY.md](../SECURITY.md).
- Never paste unreviewed third-party or local-only reference code into the public tree.

## Forbidden without owner approval

- Change `VERSION.json` `version` field.
- Publish private keys or disable updater signing.
- Change LICENSE or paid-tier rules.
- Destructive git operations (`reset --hard`, force push).

## Releases

- `releases/update.json` must match released artifacts after each release.
- Update `signature` and `url` only with signed artifacts from `tauri build`.
- See [releases/README.md](../releases/README.md).
