import { readFileSync, writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const versionFile = join(root, "VERSION.json");
const meta = JSON.parse(readFileSync(versionFile, "utf8"));
const display = `${meta.product} ${meta.version} (${meta.build})`;

/** Semver for Tauri updater: encodes monotonic build in patch (1.0.1 build 42 → 1.0.42). */
function updaterSemver(productVersion, build) {
  const [major, minor] = productVersion.split(".").map(Number);
  return `${major}.${minor}.${build}`;
}

const updaterVersion = updaterSemver(meta.version, meta.build);

const pkgPath = join(root, "minerpulse-desktop", "package.json");
const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
pkg.version = meta.version;
writeFileSync(pkgPath, `${JSON.stringify(pkg, null, 2)}\n`);

const tauriPath = join(root, "minerpulse-desktop", "src-tauri", "tauri.conf.json");
const tauri = JSON.parse(readFileSync(tauriPath, "utf8"));
tauri.version = updaterVersion;
if (!tauri.app) tauri.app = {};
tauri.app.windows ??= [{}];
tauri.app.windows[0].title = display;
writeFileSync(tauriPath, `${JSON.stringify(tauri, null, 2)}\n`);

const coreToml = join(root, "minerpulse-core", "Cargo.toml");
let toml = readFileSync(coreToml, "utf8");
toml = toml.replace(/^version = ".*"/m, `version = "${meta.version}"`);
writeFileSync(coreToml, toml);

console.log(`Synced ${display}`);
