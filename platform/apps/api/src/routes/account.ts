import { Hono } from "hono";
import { z } from "zod";
import { SubscriptionStatus, type User } from "@minerpulse/db";
import { prisma } from "../lib/prisma.js";
import { betaConfig, betaMaxDevices, betaSelfServiceEnabled } from "../lib/beta.js";
import { deleteUserDevice, maxDevicesForUser } from "../lib/device.js";
import { randomActivationCode, verifyAccessToken } from "../lib/jwt.js";
import {
  decryptPassword,
  encryptPassword,
  normalizeMac,
} from "../lib/miner-credentials-crypto.js";
import {
  readClientLogFile,
  writeClientLogFile,
} from "../lib/client-logs.js";
import { randomUUID } from "node:crypto";

type AccountEnv = { Variables: { user: User } };

const account = new Hono<AccountEnv>();
account.use("*", async (c, next) => {
  const auth = c.req.header("authorization");
  if (!auth?.startsWith("Bearer ")) {
    return c.json({ error: "unauthorized" }, 401);
  }
  try {
    const payload = await verifyAccessToken(auth.slice(7));
    const user = await prisma.user.findUnique({ where: { id: String(payload.sub) } });
    if (!user) return c.json({ error: "unauthorized" }, 401);
    c.set("user", user);
    await next();
  } catch {
    return c.json({ error: "unauthorized" }, 401);
  }
});

account.get("/me", async (c) => {
  const user = c.get("user");
  const subscription = await prisma.subscription.findFirst({
    where: { userId: user.id, status: "ACTIVE" },
    include: { plan: true },
    orderBy: { createdAt: "desc" },
  });
  const devices = await prisma.device.findMany({
    where: { userId: user.id },
    orderBy: { lastSeenAt: "desc" },
  });
  const deviceLimit = await maxDevicesForUser(user.id);
  return c.json({
    user: { id: user.id, email: user.email, nickname: user.nickname },
    subscription,
    devices,
    deviceLimit,
    deviceCount: devices.length,
    beta: betaConfig(),
  });
});

account.post("/activation-code", async (c) => {
  const user = c.get("user");
  const sub = await prisma.subscription.findFirst({
    where: { userId: user.id, status: "ACTIVE" },
  });
  if (!sub) return c.json({ error: "no_active_subscription" }, 403);

  let code = randomActivationCode();
  while (await prisma.activationCode.findUnique({ where: { code } })) {
    code = randomActivationCode();
  }
  const activation = await prisma.activationCode.create({
    data: {
      userId: user.id,
      code,
      expiresAt: new Date(Date.now() + 15 * 60 * 1000),
    },
  });
  return c.json({ code: activation.code, expires_at: activation.expiresAt });
});

account.delete("/devices/:id", async (c) => {
  const user = c.get("user");
  const id = c.req.param("id");
  const device = await prisma.device.findFirst({
    where: { id, userId: user.id },
  });
  if (!device) {
    return c.json({ error: "not_found" }, 404);
  }
  await deleteUserDevice(id);
  return c.json({ ok: true });
});

account.post("/subscribe", async (c) => {
  if (!betaSelfServiceEnabled()) {
    return c.json({ error: "beta_disabled" }, 403);
  }

  const user = c.get("user");
  const body = z
    .object({
      planId: z.string().min(1),
      deviceCount: z.number().int().min(1),
    })
    .parse(await c.req.json());

  const maxDevices = betaMaxDevices();
  if (body.deviceCount > maxDevices) {
    return c.json({ error: "device_count_too_high" }, 400);
  }

  const plan = await prisma.plan.findFirst({
    where: { id: body.planId, active: true },
  });
  if (!plan) {
    return c.json({ error: "plan_not_found" }, 404);
  }

  const deviceCount = await prisma.device.count({ where: { userId: user.id } });
  if (deviceCount > body.deviceCount) {
    return c.json({ error: "device_count_below_connected" }, 400);
  }

  const endsAt = new Date(Date.now() + plan.durationDays * 24 * 3600 * 1000);

  const subscription = await prisma.$transaction(async (tx) => {
    await tx.subscription.updateMany({
      where: { userId: user.id, status: SubscriptionStatus.ACTIVE },
      data: { status: SubscriptionStatus.CANCELLED },
    });

    await tx.user.update({
      where: { id: user.id },
      data: { maxDevicesOverride: body.deviceCount },
    });

    return tx.subscription.create({
      data: {
        userId: user.id,
        planId: plan.id,
        status: SubscriptionStatus.ACTIVE,
        source: "beta",
        endsAt,
      },
      include: { plan: true },
    });
  });

  return c.json({
    subscription,
    deviceLimit: body.deviceCount,
  });
});

const minerCredentialBody = z.object({
  mac: z.string().min(1),
  username: z.string().min(1),
  password: z.string().min(1),
});

account.get("/miner-credentials", async (c) => {
  const user = c.get("user");
  const rows = await prisma.minerCredential.findMany({
    where: { userId: user.id },
    orderBy: { updatedAt: "desc" },
  });
  return c.json({
    credentials: rows.map((row) => ({
      mac: row.mac,
      username: row.username,
      updated_at: row.updatedAt,
    })),
  });
});

account.post("/miner-credentials/sync", async (c) => {
  const user = c.get("user");
  const rows = await prisma.minerCredential.findMany({
    where: { userId: user.id },
    orderBy: { updatedAt: "desc" },
  });
  const credentials = rows.map((row) => ({
    mac: row.mac,
    username: row.username,
    password: decryptPassword(row.passwordEnc),
    updated_at: row.updatedAt,
  }));
  return c.json({ credentials });
});

account.put("/miner-credentials", async (c) => {
  const user = c.get("user");
  const body = minerCredentialBody.parse(await c.req.json());
  const mac = normalizeMac(body.mac);
  const passwordEnc = encryptPassword(body.password);
  const row = await prisma.minerCredential.upsert({
    where: { userId_mac: { userId: user.id, mac } },
    create: {
      userId: user.id,
      mac,
      username: body.username.trim(),
      passwordEnc,
    },
    update: {
      username: body.username.trim(),
      passwordEnc,
    },
  });
  return c.json({
    mac: row.mac,
    username: row.username,
    updated_at: row.updatedAt,
  });
});

account.delete("/miner-credentials/:mac", async (c) => {
  const user = c.get("user");
  const mac = normalizeMac(c.req.param("mac"));
  const existing = await prisma.minerCredential.findFirst({
    where: { userId: user.id, mac },
  });
  if (!existing) {
    return c.json({ error: "not_found" }, 404);
  }
  await prisma.minerCredential.delete({ where: { id: existing.id } });
  return c.json({ ok: true });
});

const MAX_LOG_BYTES = 8 * 1024 * 1024;

account.get("/logs", async (c) => {
  const user = c.get("user");
  const rows = await prisma.clientLog.findMany({
    where: { userId: user.id },
    orderBy: { createdAt: "desc" },
    take: 50,
  });
  return c.json({
    logs: rows.map((row) => ({
      id: row.id,
      filename: row.filename,
      hwid: row.hwid,
      size_bytes: row.sizeBytes,
      app_version: row.appVersion,
      app_build: row.appBuild,
      timezone: row.timezone,
      created_at: row.createdAt,
    })),
  });
});

account.get("/logs/:id/download", async (c) => {
  const user = c.get("user");
  const id = c.req.param("id");
  const row = await prisma.clientLog.findFirst({
    where: { id, userId: user.id },
  });
  if (!row) return c.json({ error: "not_found" }, 404);
  const bytes = await readClientLogFile(row.storagePath);
  return new Response(new Uint8Array(bytes), {
    headers: {
      "Content-Type": "application/zip",
      "Content-Disposition": `attachment; filename="${row.filename}"`,
    },
  });
});

account.post("/logs", async (c) => {
  const user = c.get("user");
  const body = await c.req.parseBody();
  const file = body.file;
  if (!(file instanceof File)) {
    return c.json({ error: "file_required" }, 400);
  }
  if (file.size <= 0 || file.size > MAX_LOG_BYTES) {
    return c.json({ error: "file_too_large" }, 400);
  }

  const filename = String(body.filename ?? file.name ?? "log.zip").trim() || "log.zip";
  const hwid = String(body.hwid ?? "").trim();
  const timezone = String(body.timezone ?? "").trim() || null;
  const appVersion = String(body.app_version ?? "").trim() || null;
  const appBuildRaw = String(body.app_build ?? "").trim();
  const appBuild = appBuildRaw ? Number.parseInt(appBuildRaw, 10) : null;

  if (hwid.length < 8) {
    return c.json({ error: "hwid_invalid" }, 400);
  }

  const bytes = Buffer.from(await file.arrayBuffer());
  const id = randomUUID();
  const storagePath = await writeClientLogFile(user.id, id, filename, bytes);
  const row = await prisma.clientLog.create({
    data: {
      id,
      userId: user.id,
      hwid,
      filename,
      sizeBytes: bytes.length,
      storagePath,
      appVersion,
      appBuild: Number.isFinite(appBuild) ? appBuild : null,
      timezone,
    },
  });

  return c.json({
    id: row.id,
    filename: row.filename,
    created_at: row.createdAt,
  });
});

// --- Per-HWID credential storage backup + shared/isolated mode ---

const MAX_BACKUP_CHARS = 512 * 1024;

account.get("/storage-mode", async (c) => {
  const user = c.get("user");
  return c.json({ shared: user.sharedStorage });
});

account.put("/storage-mode", async (c) => {
  const user = c.get("user");
  const body = z.object({ shared: z.boolean() }).parse(await c.req.json());
  await prisma.user.update({
    where: { id: user.id },
    data: { sharedStorage: body.shared },
  });
  return c.json({ shared: body.shared });
});

account.put("/storage-backup", async (c) => {
  const user = c.get("user");
  const body = z
    .object({ hwid: z.string().min(8), payload: z.string().min(1).max(MAX_BACKUP_CHARS) })
    .parse(await c.req.json());
  const payloadEnc = encryptPassword(body.payload);
  await prisma.deviceStorageBackup.upsert({
    where: { userId_hwid: { userId: user.id, hwid: body.hwid } },
    create: { userId: user.id, hwid: body.hwid, payloadEnc },
    update: { payloadEnc },
  });
  return c.json({ ok: true });
});

account.get("/storage-backups", async (c) => {
  const user = c.get("user");
  const rows = await prisma.deviceStorageBackup.findMany({
    where: { userId: user.id },
    orderBy: { updatedAt: "desc" },
  });
  return c.json({
    backups: rows.map((row) => ({
      hwid: row.hwid,
      updated_at: row.updatedAt,
      size_chars: row.payloadEnc.length,
    })),
  });
});

account.get("/storage-backup/:hwid", async (c) => {
  const user = c.get("user");
  const hwid = c.req.param("hwid");
  const row = await prisma.deviceStorageBackup.findUnique({
    where: { userId_hwid: { userId: user.id, hwid } },
  });
  if (!row) return c.json({ error: "not_found" }, 404);
  return c.json({ hwid: row.hwid, payload: decryptPassword(row.payloadEnc), updated_at: row.updatedAt });
});

export { account };
