import { Hono } from "hono";
import { prisma } from "../lib/prisma.js";
import { betaConfig } from "../lib/beta.js";

const plans = new Hono();

plans.get("/public", async (c) => {
  const items = await prisma.plan.findMany({
    where: { active: true },
    orderBy: { sortOrder: "asc" },
    select: {
      id: true,
      slug: true,
      name: true,
      tier: true,
      priceCents: true,
      currency: true,
      durationDays: true,
      maxDevices: true,
    },
  });
  return c.json({ plans: items, beta: betaConfig() });
});

export { plans };
