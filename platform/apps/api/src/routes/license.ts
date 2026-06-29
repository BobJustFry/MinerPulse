import { Hono } from "hono";
import { z } from "zod";
import { Tier } from "@minerpulse/db";
import { prisma, getOfflineGraceDays } from "../lib/prisma.js";
import { activeSubscription } from "../lib/subscription.js";
import { DeviceLimitError, findUserDevice, parseDeviceFields, upsertUserDevice } from "../lib/device.js";
import {
  hashToken,
  randomToken,
  signAccessToken,
  verifyAccessToken,
} from "../lib/jwt.js";

const license = new Hono();

const deviceFieldsSchema = z.object({
  hwid: z.string().min(8).optional(),
  device_fingerprint: z.string().min(8).optional(),
  os: z.string().min(1).optional(),
  os_version: z.string().min(1).optional(),
  app_version: z.string().optional(),
  app_build: z.coerce.number().int().positive().optional(),
});

license.post("/activate", async (c) => {
  const body = await c.req.json();
  const raw = z
    .object({
      code: z.string().min(6),
      app_version: z.string().optional(),
    })
    .merge(deviceFieldsSchema)
    .parse(body);

  const deviceInput = parseDeviceFields(raw);
  if (!deviceInput) {
    return c.json({ error: "hwid_required" }, 400);
  }
  if (raw.app_version && !deviceInput.app_version) {
    deviceInput.app_version = raw.app_version;
  }

  const activation = await prisma.activationCode.findUnique({
    where: { code: raw.code.toUpperCase() },
    include: { user: true },
  });

  if (!activation || activation.usedAt || activation.expiresAt < new Date()) {
    return c.json({ error: "invalid_code" }, 400);
  }

  const sub = await activeSubscription(activation.userId);
  if (!sub) {
    return c.json({ error: "no_active_subscription" }, 403);
  }

  let device;
  try {
    device = await upsertUserDevice(activation.userId, deviceInput, {
      maxDevices: sub.plan.maxDevices,
    });
  } catch (err) {
    if (err instanceof DeviceLimitError) {
      return c.json({ error: "device_limit" }, 403);
    }
    throw err;
  }

  await prisma.activationCode.update({
    where: { id: activation.id },
    data: { usedAt: new Date() },
  });

  const accessToken = await signAccessToken({
    sub: activation.userId,
    tier: sub.plan.tier,
    plan_id: sub.planId,
    device_id: device.id,
  });

  const refresh = randomToken(48);
  await prisma.refreshToken.create({
    data: {
      userId: activation.userId,
      deviceId: device.id,
      tokenHash: hashToken(refresh),
      expiresAt: new Date(Date.now() + 90 * 24 * 3600 * 1000),
    },
  });

  return c.json({
    access_token: accessToken,
    refresh_token: refresh,
    tier: sub.plan.tier,
    plan_name: sub.plan.name,
    expires_at: sub.endsAt,
    offline_grace_days: getOfflineGraceDays(),
  });
});

license.post("/refresh", async (c) => {
  const body = await c.req.json();
  const raw = z
    .object({ refresh_token: z.string().min(20) })
    .merge(deviceFieldsSchema)
    .parse(body);

  const deviceInput = parseDeviceFields(raw);
  if (!deviceInput) {
    return c.json({ error: "hwid_required" }, 400);
  }

  const stored = await prisma.refreshToken.findUnique({
    where: { tokenHash: hashToken(raw.refresh_token) },
  });

  if (!stored || stored.expiresAt < new Date()) {
    return c.json({ error: "invalid_refresh" }, 401);
  }

  const device = await findUserDevice(stored.userId, deviceInput.hwid);
  if (!device) {
    return c.json({ error: "device_mismatch" }, 403);
  }

  await prisma.device.update({
    where: { id: device.id },
    data: {
      lastSeenAt: new Date(),
      os: deviceInput.os,
      osVersion: deviceInput.os_version,
      appVersion: deviceInput.app_version,
      appBuild: deviceInput.app_build,
    },
  });

  const sub = await activeSubscription(stored.userId);
  if (!sub) {
    return c.json({ error: "no_active_subscription" }, 403);
  }

  const accessToken = await signAccessToken({
    sub: stored.userId,
    tier: sub.plan.tier,
    plan_id: sub.planId,
    device_id: device.id,
  });

  return c.json({
    access_token: accessToken,
    tier: sub.plan.tier,
    plan_name: sub.plan.name,
    expires_at: sub.endsAt,
    offline_grace_days: getOfflineGraceDays(),
  });
});

license.get("/status", async (c) => {
  const auth = c.req.header("authorization");
  if (!auth?.startsWith("Bearer ")) {
    return c.json({ error: "unauthorized" }, 401);
  }
  try {
    const payload = await verifyAccessToken(auth.slice(7));
    const sub = await activeSubscription(String(payload.sub));
    return c.json({
      tier: (payload.tier as Tier) ?? Tier.FREE,
      plan_id: payload.plan_id,
      subscription_active: Boolean(sub),
      expires_at: sub?.endsAt ?? null,
      offline_grace_days: getOfflineGraceDays(),
    });
  } catch {
    return c.json({ error: "invalid_token" }, 401);
  }
});

export { license };
