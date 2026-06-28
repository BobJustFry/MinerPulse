import { PrismaClient } from "@minerpulse/db";

export const prisma = new PrismaClient();

export function getEnv(name: string, fallback?: string): string {
  const value = process.env[name] ?? fallback;
  if (value == null || value === "") {
    throw new Error(`Missing env: ${name}`);
  }
  return value;
}

export function getOfflineGraceDays(): number {
  return Number(process.env.LICENSE_OFFLINE_GRACE_DAYS ?? "14");
}
