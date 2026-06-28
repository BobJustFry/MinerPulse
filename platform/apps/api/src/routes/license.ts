import { Hono } from "hono";
import { z } from "zod";
import { SubscriptionStatus, Tier } from "@minerpulse/db";
import { prisma, getOfflineGraceDays } from "../lib/prisma.js";
import {
  hashToken,
  randomActivationCode,
  randomToken,
  signAccessToken,
  verifyAccessToken,
} from "../lib/jwt.js";

const license = new Hono();

async function activeSubscription(userId: string) {
  const now = new Date();
  return prisma.subscription.findFirst({
    where: {
      userId,
      status: SubscriptionStatus.ACTIVE,
      OR: [{ endsAt: null }, { endsAt: { gt: now } }],
    },
    include: { plan: true },
    orderBy: { createdAt: "desc" },
  });
}

license.post("/activate", async (c) => {
  const body = z
    .object({
      code: z.string().min(6),
      device_fingerprint: z.string().min(8),
      app_version: z.string().optional(),
    })
    .parse(await c.req.json());

  const activation = await prisma.activationCode.findUnique({
    where: { code: body.code.toUpperCase() },
    include: { user: true },
  });

  if (!activation || activation.usedAt || activation.expiresAt < new Date()) {
    return c.json({ error: "invalid_code" }, 400);
  }

  const sub = await activeSubscription(activation.userId);
  if (!sub) {
    return c.json({ error: "no_active_subscription" }, 403);
  }

  const deviceCount = await prisma.device.count({ where: { userId: activation.userId } });
  const existing = await prisma.device.findUnique({
    where: {
      userId_fingerprint: {
        userId: activation.userId,
        fingerprint: body.device_fingerprint,
      },
    },
  });

  if (!existing && deviceCount >= sub.plan.maxDevices) {
    return c.json({ error: "device_limit" }, 403);
  }

  const device = await prisma.device.upsert({
    where: {
      userId_fingerprint: {
        userId: activation.userId,
        fingerprint: body.device_fingerprint,
      },
    },
    update: { lastSeenAt: new Date(), label: body.app_version ?? undefined },
    create: {
      userId: activation.userId,
      fingerprint: body.device_fingerprint,
      label: body.app_version,
    },
  });

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
  const body = z
    .object({ refresh_token: z.string().min(20), device_fingerprint: z.string().min(8) })
    .parse(await c.req.json());

  const stored = await prisma.refreshToken.findUnique({
    where: { tokenHash: hashToken(body.refresh_token) },
  });

  if (!stored || stored.expiresAt < new Date()) {
    return c.json({ error: "invalid_refresh" }, 401);
  }

  const device = await prisma.device.findFirst({
    where: { userId: stored.userId, fingerprint: body.device_fingerprint },
  });
  if (!device) {
    return c.json({ error: "device_mismatch" }, 403);
  }

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
