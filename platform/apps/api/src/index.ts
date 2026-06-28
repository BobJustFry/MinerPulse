import { serve } from "@hono/node-server";
import { Hono } from "hono";
import { cors } from "hono/cors";
import { auth } from "./routes/auth.js";
import { license } from "./routes/license.js";
import { plans } from "./routes/plans.js";
import { adminAuth } from "./routes/admin-auth.js";
import { admin } from "./routes/admin.js";
import { account } from "./routes/account.js";

const app = new Hono();

const webOrigin = process.env.PUBLIC_WEB_URL ?? "*";
const adminOrigin = process.env.PUBLIC_ADMIN_URL ?? "*";

app.use(
  "*",
  cors({
    origin: [webOrigin, adminOrigin],
    allowHeaders: ["Content-Type", "Authorization"],
  }),
);

app.get("/v1/health", (c) => c.json({ ok: true, service: "minerpulse-api" }));

app.route("/v1/auth", auth);
app.route("/v1/plans", plans);
app.route("/v1/license", license);
app.route("/v1/admin/auth", adminAuth);
app.route("/v1/admin", admin);
app.route("/v1/account", account);

app.onError((err, c) => {
  console.error(err);
  if (err instanceof Error && err.name === "ZodError") {
    return c.json({ error: "validation_failed" }, 400);
  }
  return c.json({ error: "internal_error" }, 500);
});

const port = Number(process.env.API_PORT ?? 3001);
console.log(`MinerPulse API listening on :${port}`);
serve({ fetch: app.fetch, port });
