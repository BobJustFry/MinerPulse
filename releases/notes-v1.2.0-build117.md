## Miner Pulse 1.2.0 (117)

### New

- **Diagnostic logging** with one-click cloud upload (About → Upload diagnostic log); download from your account or admin panel.
- **Local credential storage encrypted at rest (Windows DPAPI)** — passwords no longer stored in plaintext; auto-migrated.
- **Per-HWID cloud backup** of credentials with a **shared / isolated** toggle in the web account (multiple diagnostic PCs no longer mix data).
- **Generic cgminer fallback driver** — miners on other cgminer-family firmware now show data instead of an error.

### Fixes & improvements

- WhatsMiner now shows the real model (e.g. `M50`) and a meaningful run status instead of "Unknown".
- Antminer run status derived from telemetry (cgminer has no status field).
- WhatsMiner read: cancel in-flight jobs, fast detect path, no hangs after miner switch.
- Fixed credential store deadlock; concurrent cloud syncs are guarded.
- HWID bound to the real machine id (a copied `license.json` can no longer impersonate a device).
- Robust miner swap on the same IP (MAC/vendor change detection).
- Cloud sync failures surface in the status bar and diagnostic log.

### Install

Download **MinerPulse_1.2.117_x64-setup.exe** below (Windows x64). Existing users can update via **About → Check for updates**.
