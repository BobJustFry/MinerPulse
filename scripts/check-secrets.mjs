#!/usr/bin/env node
/**
 * Scan tracked / staged files for common secret patterns before push.
 * Usage: node scripts/check-secrets.mjs [--all]
 */
import { execSync } from "node:child_process";
import { readFileSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const scanAll = process.argv.includes("--all");

const forbiddenPaths = [
  /^\.tauri\/.*\.key$/,
  /^\.env(\.|$)/,
  /\.(pem|p12|pfx)$/i,
  /^Documents\//,
  /^OldProject\//,
];

const patterns = [
  { name: "GitHub PAT", re: /ghp_[A-Za-z0-9]{20,}/ },
  { name: "GitHub fine-grained PAT", re: /github_pat_[A-Za-z0-9_]{20,}/ },
  { name: "OpenSSH private key", re: /-----BEGIN OPENSSH PRIVATE KEY-----/ },
  { name: "RSA private key", re: /-----BEGIN RSA PRIVATE KEY-----/ },
  { name: "EC private key", re: /-----BEGIN EC PRIVATE KEY-----/ },
  { name: "Encrypted minisign secret", re: /comment: encrypted secret key/i },
  { name: "Stripe live key", re: /sk_live_[A-Za-z0-9]{16,}/ },
  { name: "Stripe test key", re: /sk_test_[A-Za-z0-9]{16,}/ },
  { name: "AWS access key", re: /AKIA[0-9A-Z]{16}/ },
  { name: "JWT hardcoded bearer", re: /Bearer eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\./ },
];

function listFiles() {
  if (scanAll) {
    return execSync("git ls-files", { cwd: root, encoding: "utf8" })
      .trim()
      .split("\n")
      .filter(Boolean);
  }
  try {
    return execSync("git diff --cached --name-only --diff-filter=ACM", {
      cwd: root,
      encoding: "utf8",
    })
      .trim()
      .split("\n")
      .filter(Boolean);
  } catch {
    return [];
  }
}

const files = listFiles();
let failed = false;

for (const rel of files) {
  for (const rule of forbiddenPaths) {
    if (rule.test(rel)) {
      console.error(`[FORBIDDEN PATH] ${rel}`);
      failed = true;
    }
  }

  const abs = join(root, rel);
  if (!existsSync(abs)) continue;

  let text;
  try {
    text = readFileSync(abs, "utf8");
  } catch {
    continue;
  }

  for (const { name, re } of patterns) {
    if (re.test(text)) {
      console.error(`[${name}] matched in ${rel}`);
      failed = true;
    }
  }
}

if (failed) {
  console.error("\nSecret scan failed. Remove secrets before commit/push.");
  process.exit(1);
}

console.log(`Secret scan OK (${files.length} file(s)).`);
