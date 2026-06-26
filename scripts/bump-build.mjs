import { readFileSync, writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const versionFile = join(root, "VERSION.json");
const meta = JSON.parse(readFileSync(versionFile, "utf8"));
meta.build += 1;
writeFileSync(versionFile, `${JSON.stringify(meta, null, 2)}\n`);

const sync = join(root, "scripts", "sync-version.mjs");
spawnSync(process.execPath, [sync], { stdio: "inherit", cwd: root });

console.log(`Build bumped to ${meta.build} (version ${meta.version} unchanged)`);
