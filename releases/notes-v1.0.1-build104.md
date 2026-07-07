## Miner Pulse 1.0.1 (104)

### Fixes

- WhatsMiner read: cooperative cancel of in-flight reads (`cancel_read_miner`)
- Fast detect path on read — skips redundant TCP round-trips before WhatsMiner fetch
- Switching miners (.33 → .35): no queued reads behind stale connection
- Auth modal reopen after Cancel without waiting for full timeout
- Driver name in status bar; themed modal close button

### Install

Download **MinerPulse_1.0.104_x64-setup.exe** (Windows x64). Update via **About → Check for updates** from build 103 or older.
