import { Hono } from "hono";
import { z } from "zod";
import { SubscriptionStatus, type User } from "@minerpulse/db";
import { prisma } from "../lib/prisma.js";
import { betaConfig, betaMaxDevices, betaSelfServiceEnabled } from "../lib/beta.js";
import { deleteUserDevice, maxDevicesForUser } from "../lib/device.js";
import { randomActivationCode, verifyAccessToken } from "../lib/jwt.js";

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

export { account };
