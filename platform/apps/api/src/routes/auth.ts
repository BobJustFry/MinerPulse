import bcrypt from "bcryptjs";
import { Hono } from "hono";
import { z } from "zod";
import { Tier } from "@minerpulse/db";
import { prisma } from "../lib/prisma.js";
import { createCaptcha, verifyCaptcha } from "../lib/captcha.js";
import { hashToken, randomToken, signAccessToken } from "../lib/jwt.js";
import { activeSubscription } from "../lib/subscription.js";
import { DeviceLimitError, parseDeviceFields, upsertUserDevice } from "../lib/device.js";

const auth = new Hono();

const nicknameSchema = z
  .string()
  .min(3)
  .max(32)
  .regex(/^[a-zA-Z0-9_]+$/, "nickname_invalid");

const registerSchema = z
  .object({
    email: z.string().email(),
    nickname: nicknameSchema,
    password: z.string().min(8),
    password_confirm: z.string().min(8),
    captcha_id: z.string().min(8),
    captcha_answer: z.union([z.string(), z.number()]),
  })
  .refine((data) => data.password === data.password_confirm, {
    message: "password_mismatch",
    path: ["password_confirm"],
  });

const loginSchema = z.object({
  email: z.string().email(),
  password: z.string().min(8),
  hwid: z.string().min(8).optional(),
  device_fingerprint: z.string().min(8).optional(),
  os: z.string().min(1).optional(),
  os_version: z.string().min(1).optional(),
  app_version: z.string().optional(),
});

async function issueUserTokens(
  userId: string,
  tier: Tier,
  opts?: { planId?: string; deviceId?: string },
) {
  const refresh = randomToken(48);
  const expiresAt = new Date(Date.now() + 30 * 24 * 3600 * 1000);
  await prisma.refreshToken.create({
    data: { userId, deviceId: opts?.deviceId, tokenHash: hashToken(refresh), expiresAt },
  });
  const accessToken = await signAccessToken({
    sub: userId,
    tier,
    plan_id: opts?.planId,
    device_id: opts?.deviceId,
  });
  return { accessToken, refresh };
}

auth.get("/captcha", (c) => {
  const captcha = createCaptcha();
  return c.json(captcha);
});

auth.post("/register", async (c) => {
  const body = registerSchema.parse(await c.req.json());
  if (!verifyCaptcha(body.captcha_id, body.captcha_answer)) {
    return c.json({ error: "captcha_failed" }, 400);
  }

  const email = body.email.toLowerCase();
  const nickname = body.nickname.toLowerCase();

  const existingEmail = await prisma.user.findUnique({ where: { email } });
  if (existingEmail) {
    return c.json({ error: "email_taken" }, 409);
  }
  const existingNick = await prisma.user.findUnique({ where: { nickname } });
  if (existingNick) {
    return c.json({ error: "nickname_taken" }, 409);
  }

  const passwordHash = await bcrypt.hash(body.password, 12);
  const user = await prisma.user.create({
    data: { email, nickname, passwordHash },
  });
  return c.json({ id: user.id, email: user.email, nickname: user.nickname });
});

auth.post("/login", async (c) => {
  const body = loginSchema.parse(await c.req.json());
  const email = body.email.toLowerCase();
  const user = await prisma.user.findUnique({ where: { email } });
  if (!user || !(await bcrypt.compare(body.password, user.passwordHash))) {
    return c.json({ error: "invalid_credentials" }, 401);
  }

  let tier: Tier = Tier.FREE;
  let planName: string | null = null;
  let expiresAt: Date | null = null;
  let planId: string | undefined;
  let deviceId: string | undefined;

  const sub = await activeSubscription(user.id);
  if (sub) {
    tier = sub.plan.tier;
    planName = sub.plan.name;
    expiresAt = sub.endsAt;
    planId = sub.planId;

    if (body.hwid || body.device_fingerprint) {
      const deviceInput = parseDeviceFields(body);
      if (deviceInput) {
        try {
          const device = await upsertUserDevice(user.id, deviceInput, {
            maxDevices: sub.plan.maxDevices,
          });
          deviceId = device.id;
        } catch (err) {
          if (err instanceof DeviceLimitError) {
            return c.json({ error: "device_limit" }, 403);
          }
          throw err;
        }
      }
    }
  }

  const { accessToken, refresh } = await issueUserTokens(user.id, tier, { planId, deviceId });

  return c.json({
    user: { id: user.id, email: user.email, nickname: user.nickname },
    access_token: accessToken,
    refresh_token: refresh,
    tier,
    plan_name: planName,
    expires_at: expiresAt,
    subscription_active: Boolean(sub),
  });
});

export { auth };
