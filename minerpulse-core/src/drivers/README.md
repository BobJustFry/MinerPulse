# Miner drivers

Each vendor (`antminer`, `avalon`, `whatsminer`) is **fully isolated**:

- Own `fetch_snapshot` logic (WhatsMiner also has `fetch_with_options` + `WhatsminerFetchOptions`)
- Own MAC module (`*/mac.rs`)
- No imports from other driver modules

`registry.rs` only detects the vendor and calls the matching driver entry point.

See also `.cursor/rules/driver-isolation.mdc` (local Cursor rule).
