## Miner Pulse 1.2.0 (133)

### New

- **Binary recordings** — `.mprs` (session) and `.mpsn` (frame); MessagePack + gzip, full snapshot data (chips, console log).
- **Fast session open** — parse once in Rust (`SessionStore`), charts immediately, frames loaded on demand during playback.
- **Unified Open** — one button auto-detects recording, snapshot, or log (legacy `.mpulse` still supported).
- **Live chart values** — cursor markers and values on poll, playback, and recording.
- **Scan favorites** — star button to save custom IP ranges, deletable from list.
- **Compact toolbar** — icon buttons with tooltips instead of labels in compact density mode.

### Fixes & improvements

- **WhatsMiner pools** on every poll/record tick (no longer skipped on fast poll).
- Removed duplicate **Pools** tab — pools stay in **Data**.
- Dev server port **5173** (Windows reserved port range fix).
- Session open no longer clears Rust cache immediately after load.

### Install

Download **MinerPulse_1.2.133_x64-setup.exe** below (Windows x64). Existing users can update via **About → Check for updates**.
