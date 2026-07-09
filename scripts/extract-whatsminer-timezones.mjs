import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const htmlPath = path.join(root, "tmp_luci_system.html");
const outPath = path.join(root, "minerpulse-desktop/src/lib/whatsminerTimezones.json");

const html = fs.readFileSync(htmlPath, "utf8");
const match = html.match(/<select[^>]*zonename[^>]*>([\s\S]*?)<\/select>/);
if (!match) {
  console.error("zonename select not found");
  process.exit(1);
}
const zones = [...match[1].matchAll(/value="([^"]+)"/g)].map((m) => m[1]);
const unique = [...new Set(zones)].sort((a, b) => a.localeCompare(b));
fs.writeFileSync(outPath, JSON.stringify(unique, null, 2) + "\n");
console.log(`Wrote ${unique.length} zones to ${outPath}`);
