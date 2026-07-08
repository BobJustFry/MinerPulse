## Miner Pulse 1.2.0 (123)

### New

- **Extended miner parameters** — PSU (input/output voltage, current, power, temp, fan, model), power mode, frequency, rated hashrate, power limit, cooling type, chip temperatures and error rates. Shown as cards (normal density) or a collapsible block (compact).
- **Generic cgminer fallback driver** — miners on other cgminer-family firmware now show data instead of an error.
- **WhatsMiner pools** now shown on read; extended pool fields (priority, stratum, difficulty, stale%).

### Fixes & improvements

- Chips tab hidden when a miner has no chip data — no flicker during poll/record.
- Chip section stays visible in the console between full poll refreshes.
- WhatsMiner shows real model and a meaningful run status; Antminer run status derived from telemetry.
- Local credentials encrypted at rest (Windows DPAPI); per-HWID cloud backup with shared/isolated mode.
- Diagnostic logging with cloud upload; robust miner-swap handling on the same IP.
- Fixed credential store deadlock; hardened HWID binding.

### Install

Download **MinerPulse_1.2.123_x64-setup.exe** below (Windows x64). Existing users can update via **About → Check for updates**.
