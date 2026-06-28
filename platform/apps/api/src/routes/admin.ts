import { Hono } from "hono";
import { z } from "zod";
import { SubscriptionStatus, Tier, type AdminUser } from "@minerpulse/db";
import { prisma } from "../lib/prisma.js";
import { randomActivationCode, verifyAccessToken } from "../lib/jwt.js";

type AdminEnv = { Variables: { admin: AdminUser } };

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

admin.get("/users", async (c) => {
  const users = await prisma.user.findMany({
    orderBy: { createdAt: "desc" },
    include: {
      subscriptions: { include: { plan: true }, orderBy: { createdAt: "desc" }, take: 1 },
      _count: { select: { devices: true } },
    },
  });
  return c.json({ users });
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
      name: z.string().min(2).optional(),
      priceCents: z.number().int().nonnegative().optional(),
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

export { admin };
