import bcrypt from "bcryptjs";
import { Hono } from "hono";
import { z } from "zod";
import { Tier } from "@minerpulse/db";
import { prisma } from "../lib/prisma.js";
import { signAccessToken } from "../lib/jwt.js";

const adminAuth = new Hono();

adminAuth.post("/login", async (c) => {
  const body = z
    .object({ username: z.string().min(3).max(64), password: z.string().min(8) })
    .parse(await c.req.json());
  const admin = await prisma.adminUser.findUnique({
    where: { username: body.username },
  });
  if (!admin || !(await bcrypt.compare(body.password, admin.passwordHash))) {
    return c.json({ error: "invalid_credentials" }, 401);
  }
  const token = await signAccessToken(
    { sub: admin.id, tier: Tier.SERVICE, admin_role: admin.role },
    8 * 3600,
  );
  return c.json({
    access_token: token,
    admin: { id: admin.id, username: admin.username, role: admin.role },
  });
});

export { adminAuth };
