import { createHash, randomBytes } from "node:crypto";
import { SignJWT, jwtVerify, importPKCS8, importSPKI } from "jose";
import type { Tier } from "@minerpulse/db";
import { getEnv } from "./prisma.js";

let privateKey: CryptoKey | null = null;
let publicKey: CryptoKey | null = null;

async function loadKeys() {
  if (!privateKey) {
    privateKey = await importPKCS8(pemFromEnv("JWT_PRIVATE_KEY"), "RS256");
  }
  if (!publicKey) {
    publicKey = await importSPKI(pemFromEnv("JWT_PUBLIC_KEY"), "RS256");
  }
  return { privateKey, publicKey };
}

function pemFromEnv(name: string): string {
  return getEnv(name).replace(/\\n/g, "\n");
}

export type LicenseClaims = {
  sub: string;
  tier: Tier;
  plan_id?: string;
  device_id?: string;
  admin_role?: string;
};

export async function signAccessToken(claims: LicenseClaims, expiresInSec = 3600) {
  const { privateKey: key } = await loadKeys();
  const payload: Record<string, string> = { tier: claims.tier };
  if (claims.plan_id) payload.plan_id = claims.plan_id;
  if (claims.device_id) payload.device_id = claims.device_id;
  if (claims.admin_role) payload.admin_role = claims.admin_role;
  return new SignJWT(payload)
    .setProtectedHeader({ alg: "RS256" })
    .setSubject(claims.sub)
    .setIssuedAt()
    .setExpirationTime(`${expiresInSec}s`)
    .sign(key);
}

export async function verifyAccessToken(token: string) {
  const { publicKey: key } = await loadKeys();
  const { payload } = await jwtVerify(token, key);
  return payload;
}

export function hashToken(token: string): string {
  return createHash("sha256").update(token).digest("hex");
}

export function randomToken(bytes = 32): string {
  return randomBytes(bytes).toString("base64url");
}

export function randomActivationCode(): string {
  return randomBytes(6).toString("hex").toUpperCase();
}
