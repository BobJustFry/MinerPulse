import bcrypt from "bcryptjs";
import { Hono } from "hono";
import { z } from "zod";
import { prisma } from "../lib/prisma.js";
import { hashToken, randomToken, signAccessToken } from "../lib/jwt.js";
import { Tier } from "@minerpulse/db";

const auth = new Hono();

const registerSchema = z.object({
  email: z.string().email(),
  password: z.string().min(8),
});

auth.post("/register", async (c) => {
  const body = registerSchema.parse(await c.req.json());
  const email = body.email.toLowerCase();
  const existing = await prisma.user.findUnique({ where: { email } });
  if (existing) {
    return c.json({ error: "email_taken" }, 409);
  }
  const passwordHash = await bcrypt.hash(body.password, 12);
  const user = await prisma.user.create({ data: { email, passwordHash } });
  return c.json({ id: user.id, email: user.email });
});

auth.post("/login", async (c) => {
  const body = registerSchema.parse(await c.req.json());
  const email = body.email.toLowerCase();
  const user = await prisma.user.findUnique({ where: { email } });
  if (!user || !(await bcrypt.compare(body.password, user.passwordHash))) {
    return c.json({ error: "invalid_credentials" }, 401);
  }
  const refresh = randomToken(48);
  const expiresAt = new Date(Date.now() + 30 * 24 * 3600 * 1000);
  await prisma.refreshToken.create({
    data: { userId: user.id, tokenHash: hashToken(refresh), expiresAt },
  });
  const accessToken = await signAccessToken({ sub: user.id, tier: Tier.FREE });
  return c.json({
    user: { id: user.id, email: user.email },
    access_token: accessToken,
    refresh_token: refresh,
  });
});

export { auth };
