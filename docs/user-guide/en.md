<p align="center">
  <a href="README.md">📖 Guide home</a>
  &nbsp;·&nbsp;
  <a href="ru.md">🇷🇺 Русский</a>
  &nbsp;·&nbsp;
  <strong>🇺🇸 English</strong>
  &nbsp;·&nbsp;
  <a href="zh-CN.md">🇨🇳 中文</a>
</p>

# Miner Pulse User Guide

**Miner Pulse** is a Windows desktop app for monitoring ASIC miners over the network (WhatsMiner, Antminer, Avalon, and other cgminer-compatible devices).

Version and build number are shown in the window title: `Miner Pulse X.Y.Z (BBB)`.

---

## Contents

1. [Installation](#installation)
2. [Quick start](#quick-start)
3. [Toolbar](#toolbar)
4. [Tabs](#tabs)
5. [Recordings and files](#recordings-and-files)
6. [Network scan](#network-scan)
7. [Supported miners](#supported-miners)
8. [WhatsMiner: first connection](#whatsminer-first-connection)
9. [Subscription](#subscription)
10. [Updates](#updates)
11. [Tips & troubleshooting](#tips--troubleshooting)

---

## Installation

1. Open [Releases](https://github.com/BobJustFry/MinerPulse/releases/latest).
2. Download **`MinerPulse_*_x64-setup.exe`**.
3. Run the installer (NSIS).
4. Launch **Miner Pulse**.

> On first run, Windows may ask for firewall permission — allow local network access for polling and scan to work.

---

## Quick start

1. Enter the miner **IP** and **port** (default `4028`).
2. Click **Read** for a one-shot telemetry fetch.
3. Data appears on the **Data** tab; **Chips** appears when chip data is available.

For continuous monitoring use **Poll** (see below). WhatsMiner may show a setup dialog on first connect — see [WhatsMiner](#whatsminer-first-connection).

---

## Toolbar

| Control | Description |
|---------|-------------|
| **IP : Port** | Miner address. cgminer API port is usually `4028`. |
| **Scan** | Scan a subnet and pick a miner from the list. |
| **Read** | Single read without background polling. |
| **Poll** | Periodic polling at 1–15 Hz. |
| **Record** | Poll and save a session to `.mprs`. |
| **Stop** | Stop poll or recording. |
| **Open** | Open recording, snapshot, log, or legacy `.mpulse` (auto-detected). |
| **Save** | Save current snapshot as `.mpsn`. |
| **Theme** | Light / dark. |
| **Compact** | Compact toolbar: icons with tooltips on hover. |
| **Language** | RU / EN / 中文. |
| **Subscription** | Manage tier (tier badge). |
| **About** | Version, updates, donate, diagnostics. |

### Action modes (split button ▾)

- **Read** — one-shot.
- **Poll** — live charts and refreshed data.
- **Record** — poll with file recording (Client or Service tier).

When **Poll** or **Record** is selected, choose **poll rate** (1/3/5/10/15 per second).

---

## Tabs

### Data

Overview: model, firmware, status, hashrate, temps, fans, power, boards, pools, extended params (PSU, mode, frequency, etc.).

### Chips

Per-board chip matrix (temperature, voltage, errors). Hidden when the miner provides no chip map.

### Console

Raw log and API responses for debugging.

### Charts

Hashrate, board temps, power, fan RPM. Live during poll/record; scrubber during playback.

Layouts: **tile** or **list**.

### Commands

Miner control (reboot, frequency, work mode, etc.) — **Avalon only**, Client/Service tier.

---

## Recordings and files

| Extension | Type | Description |
|-----------|------|-------------|
| `.mprs` | Session | Full poll history: all fields, chips, console. |
| `.mpsn` | Snapshot | Single point in time. |
| `.mpulse` | Legacy | Old JSON format; still supported. |
| `.txt` / `.log` | Log | Import Avalon/Antminer logs. |

### Playback

1. **Open** → select `.mprs`.
2. **Charts** tab opens with the playback bar at the bottom.
3. **Play** / **Pause**, seek, speed 0.5×–8×.
4. **Data**, **Chips**, **Console** reflect the current frame.

### Drag & drop

Text logs (`.txt`, `.log`) can be dropped onto the window. Open binary `.mprs` via **Open** button.

---

## Network scan

1. Click **Scan** on the main toolbar.
2. Pick a subnet or **custom IP range**.
3. **★** saves the range to **Favorites**.
4. **Scan** runs discovery; double-click a row to use that IP.

Fully supported in the list: Avalon, Antminer, WhatsMiner. Other cgminer hosts may appear with limited support.

---

## Supported miners

| Family | Full read | Chips | Commands |
|--------|:---------:|:-----:|:--------:|
| **Avalon** (Canaan) | ✅ | ✅ | ✅ |
| **Antminer** (Bitmain) | ✅ | ✅* | — |
| **WhatsMiner** (MicroBT) | ✅ | ✅** | — |
| **Other cgminer** (LuxOS, Braiins, etc.) | ⚠️ basic | — | — |
| **Innosilicon** | ❌ | — | — |

\* Model/firmware dependent.  
\** WhatsMiner chips may need LuCI/API setup.

**Custom firmware:** if the miner speaks cgminer on port 4028 but is not matched as Antminer/Avalon/WhatsMiner, you get **basic** telemetry (hashrate, pools, temps) — no chip map or commands.

---

## WhatsMiner: first connection

1. Click **Read** or **Poll**.
2. If setup is required, the **WhatsMiner setup** dialog appears:
   - LuCI username/password (default `admin` / `admin`);
   - **Test login**;
   - **Save and read**.
3. Credentials can sync to the cloud when signed in.

Without LuCI/API, chips may be empty while hashrate and pools still work via TCP API.

---

## Subscription

| Tier | Poll / record | Charts | Playback | Commands |
|------|:-------------:|:------:|:--------:|:--------:|
| **Free** | Single read (≥10 s interval) | — | — | — |
| **Client** | Poll + `.mprs` recording | ✅ | ✅ | ✅ (Avalon) |
| **Service** | Same | ✅ | ✅ | ✅ |

Activate via tier badge → **Manage subscription** → code from [mpulse.bob4.fun](https://mpulse.bob4.fun) or email login.

---

## Updates

**About** → **Check for updates**.

Updates are cryptographically signed and delivered from GitHub Releases.

---

## Tips & troubleshooting

| Issue | Try |
|-------|-----|
| No response | Check network, ping, port 4028, Windows firewall. |
| Long “Reading…” (WhatsMiner) | Wait up to 15 s or **Cancel** and retry; test LuCI in browser. |
| No chips | WhatsMiner: API setup; Antminer/Avalon: firmware may not expose devs. |
| Recording won’t open | Use **Open** (not drag-drop for `.mprs`). Client+ tier required. |
| License error | Check internet and device limit on the website. |

**Diagnostic log:** About → send log (account required). View in the web portal.

---

<p align="center">
  <a href="README.md">📖 Guide home</a>
  &nbsp;·&nbsp;
  <a href="ru.md">🇷🇺 Русский</a>
  &nbsp;·&nbsp;
  <a href="zh-CN.md">🇨🇳 中文</a>
  &nbsp;·&nbsp;
  <a href="https://t.me/miner_pulse">Telegram</a>
</p>
