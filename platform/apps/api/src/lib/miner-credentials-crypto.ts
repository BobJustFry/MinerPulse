import { createCipheriv, createDecipheriv, randomBytes } from "node:crypto";

const IV_LEN = 12;
const TAG_LEN = 16;

function keyFromEnv(): Buffer {
  const raw = process.env.MINER_CREDENTIALS_KEY;
  if (!raw) {
    throw new Error("MINER_CREDENTIALS_KEY missing");
  }
  const key = Buffer.from(raw, "base64");
  if (key.length !== 32) {
    throw new Error("MINER_CREDENTIALS_KEY must be 32 bytes base64");
  }
  return key;
}

export function encryptPassword(plaintext: string): string {
  const key = keyFromEnv();
  const iv = randomBytes(IV_LEN);
  const cipher = createCipheriv("aes-256-gcm", key, iv);
  const enc = Buffer.concat([cipher.update(plaintext, "utf8"), cipher.final()]);
  const tag = cipher.getAuthTag();
  return Buffer.concat([iv, tag, enc]).toString("base64");
}

export function decryptPassword(blob: string): string {
  const key = keyFromEnv();
  const data = Buffer.from(blob, "base64");
  if (data.length < IV_LEN + TAG_LEN + 1) {
    throw new Error("invalid_ciphertext");
  }
  const iv = data.subarray(0, IV_LEN);
  const tag = data.subarray(IV_LEN, IV_LEN + TAG_LEN);
  const enc = data.subarray(IV_LEN + TAG_LEN);
  const decipher = createDecipheriv("aes-256-gcm", key, iv);
  decipher.setAuthTag(tag);
  return Buffer.concat([decipher.update(enc), decipher.final()]).toString("utf8");
}

export function normalizeMac(raw: string): string {
  return raw.trim().toUpperCase().replace(/-/g, ":");
}
