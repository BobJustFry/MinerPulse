import bcrypt from "bcryptjs";
import { Hono } from "hono";
import { z } from "zod";
import { SubscriptionStatus, Tier, type AdminUser } from "@minerpulse/db";
import { prisma } from "../lib/prisma.js";
import {
  DeviceLimitError,
  createUserDevice,
  deleteUserDevice,
  effectiveMaxDevices,
  parseDeviceFields,
} from "../lib/device.js";
import { activeSubscription } from "../lib/subscription.js";
import { randomActivationCode, verifyAccessToken } from "../lib/jwt.js";
import { readClientLogFile } from "../lib/client-logs.js";

type AdminEnv = { Variables: { admin: AdminUser } };

const nicknameSchema = z
  .string()
  .min(3)
  .max(32)
  .regex(/^[a-zA-Z0-9_]+$/);

const admin = new Hono<AdminEnv>();
admin.use("*", async (c, next) => {
  const auth = c.req.header("authorization");
  if (!auth?.startsWith("Bearer ")) {
    return c.json({ error: "unauthorized" }, 401);
  }
  try {
    const payload = await verifyAccessToken(auth.slice(7));
    const adminUser = await prisma.adminUser.findUnique({
      where: { id: String(payload.sub) },
    });
    if (!adminUser) return c.json({ error: "unauthorized" }, 401);
    if (adminUser.role !== "SUPER_ADMIN" && adminUser.role !== "ADMIN") {
      return c.json({ error: "forbidden" }, 403);
    }
    c.set("admin", adminUser);
    await next();
  } catch {
    return c.json({ error: "unauthorized" }, 401);
  }
});

async function audit(
  c: { get: (key: "admin") => AdminUser },
  action: string,
  entity: string,
  entityId?: string,
  payload?: unknown,
) {
  const adminUser = c.get("admin");
  await prisma.auditLog.create({
    data: { adminId: adminUser.id, action, entity, entityId, payload: payload as object },
  });
}

async function userDeviceLimit(user: {
  maxDevicesOverride: number | null;
  subscriptions: Array<{ plan: { maxDevices: number } }>;
  _count?: { devices: number };
}) {
  const planMax = user.subscriptions[0]?.plan.maxDevices ?? 1;
  const limit = effectiveMaxDevices(planMax, user.maxDevicesOverride);
  return { planMax, limit, count: user._count?.devices ?? 0 };
}

admin.get("/users", async (c) => {
  const users = await prisma.user.findMany({
    orderBy: { createdAt: "desc" },
    include: {
      subscriptions: { include: { plan: true }, orderBy: { createdAt: "desc" }, take: 1 },
      _count: { select: { devices: true } },
    },
  });
  const enriched = await Promise.all(
    users.map(async (user) => {
      const { planMax, limit, count } = await userDeviceLimit(user);
      return { ...user, deviceLimit: limit, devicePlanMax: planMax, deviceCount: count };
    }),
  );
  return c.json({ users: enriched });
});

admin.get("/users/:id", async (c) => {
  const user = await prisma.user.findUnique({
    where: { id: c.req.param("id") },
    include: {
      subscriptions: { include: { plan: true }, orderBy: { createdAt: "desc" } },
      devices: { orderBy: { lastSeenAt: "desc" } },
      _count: { select: { devices: true } },
    },
  });
  if (!user) return c.json({ error: "not_found" }, 404);
  const { planMax, limit, count } = await userDeviceLimit(user);
  return c.json({
    user: {
      ...user,
      deviceLimit: limit,
      devicePlanMax: planMax,
      deviceCount: count,
    },
  });
});

admin.post("/users", async (c) => {
  const body = z
    .object({
      email: z.string().email(),
      nickname: nicknameSchema,
      password: z.string().min(8),
      locale: z.string().default("ru"),
    })
    .parse(await c.req.json());
  const email = body.email.toLowerCase();
  const nickname = body.nickname.toLowerCase();
  if (await prisma.user.findUnique({ where: { email } })) {
    return c.json({ error: "email_taken" }, 409);
  }
  if (await prisma.user.findUnique({ where: { nickname } })) {
    return c.json({ error: "nickname_taken" }, 409);
  }
  const passwordHash = await bcrypt.hash(body.password, 12);
  const user = await prisma.user.create({
    data: { email, nickname, passwordHash, locale: body.locale },
  });
  await audit(c, "create", "User", user.id, { email, nickname });
  return c.json({ user });
});

admin.patch("/users/:id", async (c) => {
  const id = c.req.param("id");
  const body = z
    .object({
      email: z.string().email().optional(),
      nickname: nicknameSchema.optional(),
      password: z.string().min(8).optional(),
      locale: z.string().optional(),
      maxDevicesOverride: z.number().int().positive().nullable().optional(),
    })
    .parse(await c.req.json());

  const data: {
    email?: string;
    nickname?: string;
    passwordHash?: string;
    locale?: string;
    maxDevicesOverride?: number | null;
  } = {};

  if (body.email) {
    const email = body.email.toLowerCase();
    const clash = await prisma.user.findFirst({ where: { email, NOT: { id } } });
    if (clash) return c.json({ error: "email_taken" }, 409);
    data.email = email;
  }
  if (body.nickname) {
    const nickname = body.nickname.toLowerCase();
    const clash = await prisma.user.findFirst({ where: { nickname, NOT: { id } } });
    if (clash) return c.json({ error: "nickname_taken" }, 409);
    data.nickname = nickname;
  }
  if (body.password) {
    data.passwordHash = await bcrypt.hash(body.password, 12);
  }
  if (body.locale) data.locale = body.locale;
  if (body.maxDevicesOverride !== undefined) {
    data.maxDevicesOverride = body.maxDevicesOverride;
  }

  const user = await prisma.user.update({ where: { id }, data });
  await audit(c, "update", "User", id, body);
  return c.json({ user });
});

admin.delete("/users/:id", async (c) => {
  const id = c.req.param("id");
  await prisma.user.delete({ where: { id } });
  await audit(c, "delete", "User", id);
  return c.json({ ok: true });
});

admin.post("/users/:userId/devices", async (c) => {
  const userId = c.req.param("userId");
  const user = await prisma.user.findUnique({ where: { id: userId } });
  if (!user) return c.json({ error: "not_found" }, 404);

  const body = z
    .object({
      hwid: z.string().min(8),
      label: z.string().max(64).nullable().optional(),
      os: z.string().optional(),
      os_version: z.string().optional(),
      app_version: z.string().optional(),
      app_build: z.coerce.number().int().positive().optional(),
    })
    .parse(await c.req.json());

  const deviceInput = parseDeviceFields(body);
  if (!deviceInput) {
    return c.json({ error: "hwid_invalid" }, 400);
  }

  const sub = await activeSubscription(userId);
  const planMax = sub?.plan.maxDevices ?? 1;
  const maxDevices = effectiveMaxDevices(planMax, user.maxDevicesOverride);

  try {
    const device = await createUserDevice(userId, deviceInput, {
      maxDevices,
      label: body.label ?? null,
    });
    await audit(c, "create", "Device", device.id, { userId, hwid: device.hwid });
    return c.json({ device });
  } catch (err) {
    if (err instanceof DeviceLimitError) {
      return c.json({ error: "device_limit", max_devices: maxDevices }, 403);
    }
    throw err;
  }
});

admin.patch("/devices/:id", async (c) => {
  const id = c.req.param("id");
  const body = z
    .object({
      label: z.string().max(64).nullable().optional(),
      hwid: z.string().min(8).optional(),
    })
    .parse(await c.req.json());

  const existing = await prisma.device.findUnique({ where: { id } });
  if (!existing) return c.json({ error: "not_found" }, 404);

  if (body.hwid && body.hwid !== existing.hwid) {
    const clash = await prisma.device.findUnique({
      where: { userId_hwid: { userId: existing.userId, hwid: body.hwid } },
    });
    if (clash && clash.id !== id) {
      return c.json({ error: "hwid_taken" }, 409);
    }
  }

  const device = await prisma.device.update({
    where: { id },
    data: {
      label: body.label,
      hwid: body.hwid,
    },
  });
  await audit(c, "update", "Device", id, body);
  return c.json({ device });
});

admin.delete("/devices/:id", async (c) => {
  const id = c.req.param("id");
  const device = await prisma.device.findUnique({ where: { id } });
  if (!device) return c.json({ error: "not_found" }, 404);

  await deleteUserDevice(id);
  await audit(c, "delete", "Device", id, { userId: device.userId, hwid: device.hwid });
  return c.json({ ok: true });
});

admin.get("/plans", async (c) => {
  const plans = await prisma.plan.findMany({ orderBy: { sortOrder: "asc" } });
  return c.json({ plans });
});

admin.post("/plans", async (c) => {
  const body = z
    .object({
      slug: z.string().min(2),
      name: z.string().min(2),
      tier: z.nativeEnum(Tier),
      priceCents: z.number().int().nonnegative(),
      currency: z.string().default("RUB"),
      durationDays: z.number().int().positive(),
      maxDevices: z.number().int().positive().default(1),
      active: z.boolean().default(true),
      sortOrder: z.number().int().default(0),
    })
    .parse(await c.req.json());
  const plan = await prisma.plan.create({ data: body });
  await audit(c, "create", "Plan", plan.id, body);
  return c.json({ plan });
});

admin.patch("/plans/:id", async (c) => {
  const id = c.req.param("id");
  const body = z
    .object({
      slug: z.string().min(2).optional(),
      name: z.string().min(2).optional(),
      tier: z.nativeEnum(Tier).optional(),
      priceCents: z.number().int().nonnegative().optional(),
      currency: z.string().optional(),
      durationDays: z.number().int().positive().optional(),
      maxDevices: z.number().int().positive().optional(),
      active: z.boolean().optional(),
      sortOrder: z.number().int().optional(),
    })
    .parse(await c.req.json());
  const plan = await prisma.plan.update({ where: { id }, data: body });
  await audit(c, "update", "Plan", id, body);
  return c.json({ plan });
});

admin.delete("/plans/:id", async (c) => {
  const id = c.req.param("id");
  const subs = await prisma.subscription.count({ where: { planId: id } });
  if (subs > 0) {
    const plan = await prisma.plan.update({ where: { id }, data: { active: false } });
    await audit(c, "deactivate", "Plan", id);
    return c.json({ plan, deactivated: true });
  }
  await prisma.plan.delete({ where: { id } });
  await audit(c, "delete", "Plan", id);
  return c.json({ ok: true });
});

admin.post("/subscriptions", async (c) => {
  const body = z
    .object({
      userId: z.string(),
      planId: z.string(),
      endsAt: z.string().datetime().optional(),
      source: z.string().default("manual"),
    })
    .parse(await c.req.json());
  const plan = await prisma.plan.findUniqueOrThrow({ where: { id: body.planId } });
  const endsAt = body.endsAt
    ? new Date(body.endsAt)
    : new Date(Date.now() + plan.durationDays * 24 * 3600 * 1000);
  const subscription = await prisma.subscription.create({
    data: {
      userId: body.userId,
      planId: body.planId,
      status: SubscriptionStatus.ACTIVE,
      source: body.source,
      endsAt,
    },
    include: { plan: true, user: true },
  });
  await audit(c, "create", "Subscription", subscription.id, body);
  return c.json({ subscription });
});

admin.patch("/subscriptions/:id", async (c) => {
  const id = c.req.param("id");
  const body = z
    .object({
      status: z.nativeEnum(SubscriptionStatus).optional(),
      endsAt: z.string().datetime().nullable().optional(),
    })
    .parse(await c.req.json());
  const subscription = await prisma.subscription.update({
    where: { id },
    data: {
      status: body.status,
      endsAt: body.endsAt === undefined ? undefined : body.endsAt ? new Date(body.endsAt) : null,
    },
    include: { plan: true, user: true },
  });
  await audit(c, "update", "Subscription", id, body);
  return c.json({ subscription });
});

admin.post("/activation-codes", async (c) => {
  const body = z.object({ userId: z.string(), ttlMinutes: z.number().int().positive().default(15) }).parse(
    await c.req.json(),
  );
  let code = randomActivationCode();
  while (await prisma.activationCode.findUnique({ where: { code } })) {
    code = randomActivationCode();
  }
  const activation = await prisma.activationCode.create({
    data: {
      userId: body.userId,
      code,
      expiresAt: new Date(Date.now() + body.ttlMinutes * 60 * 1000),
    },
  });
  await audit(c, "create", "ActivationCode", activation.id, { userId: body.userId });
  return c.json({ code: activation.code, expires_at: activation.expiresAt });
});

admin.get("/audit", async (c) => {
  const logs = await prisma.auditLog.findMany({
    orderBy: { createdAt: "desc" },
    take: 200,
    include: { admin: { select: { username: true, email: true } } },
  });
  return c.json({ logs });
});

admin.get("/client-logs", async (c) => {
  const userId = c.req.query("userId");
  const hwid = c.req.query("hwid");
  const rows = await prisma.clientLog.findMany({
    where: {
      ...(userId ? { userId } : {}),
      ...(hwid ? { hwid: { contains: hwid } } : {}),
    },
    orderBy: { createdAt: "desc" },
    take: 200,
    include: { user: { select: { email: true, nickname: true } } },
  });
  return c.json({
    logs: rows.map((row) => ({
      id: row.id,
      user_id: row.userId,
      user_email: row.user.email,
      user_nickname: row.user.nickname,
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

admin.get("/client-logs/:id/download", async (c) => {
  const id = c.req.param("id");
  const row = await prisma.clientLog.findUnique({ where: { id } });
  if (!row) return c.json({ error: "not_found" }, 404);
  const bytes = await readClientLogFile(row.storagePath);
  return new Response(bytes, {
    headers: {
      "Content-Type": "application/zip",
      "Content-Disposition": `attachment; filename="${row.filename}"`,
    },
  });
});

export { admin };
