import { randomBytes } from "node:crypto";

type CaptchaEntry = { answer: number; expiresAt: number };

const store = new Map<string, CaptchaEntry>();
const TTL_MS = 10 * 60 * 1000;

function purgeExpired() {
  const now = Date.now();
  for (const [id, entry] of store) {
    if (entry.expiresAt <= now) store.delete(id);
  }
}

export function createCaptcha() {
  purgeExpired();
  const a = Math.floor(Math.random() * 9) + 1;
  const b = Math.floor(Math.random() * 9) + 1;
  const id = randomBytes(12).toString("hex");
  store.set(id, { answer: a + b, expiresAt: Date.now() + TTL_MS });
  return { id, question: `${a} + ${b}` };
}

export function verifyCaptcha(id: string, answer: string | number) {
  purgeExpired();
  const entry = store.get(id);
  if (!entry || entry.expiresAt <= Date.now()) {
    return false;
  }
  store.delete(id);
  const parsed = typeof answer === "number" ? answer : Number.parseInt(String(answer).trim(), 10);
  return Number.isFinite(parsed) && parsed === entry.answer;
}
