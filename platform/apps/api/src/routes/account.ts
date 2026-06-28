import { Hono } from "hono";
import { z } from "zod";
import type { User } from "@minerpulse/db";
import { prisma } from "../lib/prisma.js";
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
  const devices = await prisma.device.findMany({ where: { userId: user.id } });
  return c.json({ user: { id: user.id, email: user.email }, subscription, devices });
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

export { account };
