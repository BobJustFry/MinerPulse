/**
 * Import WhatsMiner hashboard layout table from HashSource/whatsminer_chip_map.
 * Source: https://github.com/HashSource/whatsminer_chip_map (GPL-3.0, firmware-derived data)
 *
 * Usage: node scripts/import-whatsminer-layouts.mjs
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const sourceUrl =
  "https://raw.githubusercontent.com/HashSource/whatsminer_chip_map/master/src/config.rs";
const outPath = path.join(
  root,
  "minerpulse-core/src/drivers/whatsminer/layout_configs.json",
);

const source = fs.existsSync(path.join(root, "support/whatsminer_chip_map/config.rs"))
  ? fs.readFileSync(path.join(root, "support/whatsminer_chip_map/config.rs"), "utf8")
  : await fetch(sourceUrl).then((r) => {
      if (!r.ok) throw new Error(`fetch failed: ${r.status}`);
      return r.text();
    });

const blockRe = /MinerConfig\s*\{([^}]+)\}/gs;
/** @type {Array<{model:string,chip_num:number,chips_per_domain:number,board_num:number,slot_link:string|null}>} */
const configs = [];

for (const match of source.matchAll(blockRe)) {
  const block = match[1];
  const model = block.match(/model:\s*"([^"]+)"/)?.[1];
  const chipNum = block.match(/chip_num:\s*(\d+)/)?.[1];
  const cpd = block.match(/chips_per_domain:\s*(\d+)/)?.[1];
  const boardNum = block.match(/board_num:\s*(\d+)/)?.[1];
  const slotLink = block.match(/slot_link:\s*Some\("([^"]+)"\)/)?.[1] ?? null;
  if (!model || !chipNum || !cpd || !boardNum) continue;
  configs.push({
    model,
    chip_num: Number(chipNum),
    chips_per_domain: Number(cpd),
    board_num: Number(boardNum),
    slot_link: slotLink,
  });
}

if (configs.length < 100) {
  throw new Error(`expected 100+ configs, got ${configs.length}`);
}

const payload = {
  attribution:
    "Layout parameters derived from HashSource/whatsminer_chip_map (GPL-3.0), MicroBT firmware tables.",
  source_url: sourceUrl,
  config_count: configs.length,
  configs,
};

fs.writeFileSync(outPath, `${JSON.stringify(payload, null, 2)}\n`, "utf8");
console.log(`Wrote ${configs.length} layouts to ${outPath}`);
