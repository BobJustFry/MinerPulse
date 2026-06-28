import bcrypt from "bcryptjs";
import { Hono } from "hono";
import { z } from "zod";
import { prisma } from "../lib/prisma.js";
import { hashToken, randomToken, signAccessToken } from "../lib/jwt.js";

const adminAuth = new Hono();

adminAuth.post("/login", async (c) => {
  const body = z
    .object({ email: z.string().email(), password: z.string().min(8) })
    .parse(await c.req.json());
  const admin = await prisma.adminUser.findUnique({
    where: { email: body.email.toLowerCase() },
  });
  if (!admin || !(await bcrypt.compare(body.password, admin.passwordHash))) {
    return c.json({ error: "invalid_credentials" }, 401);
  }
  const token = await signAccessToken(
    { sub: admin.id, tier: "SERVICE" as const },
    8 * 3600,
  );
  return c.json({ access_token: token, admin: { id: admin.id, email: admin.email } });
});

export { adminAuth };
