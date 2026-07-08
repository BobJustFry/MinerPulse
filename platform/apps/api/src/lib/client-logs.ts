import fs from "node:fs/promises";
import path from "node:path";

export function clientLogRoot(): string {
  return process.env.CLIENT_LOG_DIR ?? path.join(process.cwd(), "data", "client-logs");
}

export async function ensureClientLogRoot(): Promise<string> {
  const root = clientLogRoot();
  await fs.mkdir(root, { recursive: true });
  return root;
}

export function clientLogStoragePath(userId: string, id: string, filename: string): string {
  const safe = filename.replace(/[^a-zA-Z0-9._-]/g, "_");
  return path.join(userId, `${id}_${safe}`);
}

export async function writeClientLogFile(
  userId: string,
  id: string,
  filename: string,
  bytes: Buffer,
): Promise<string> {
  const root = await ensureClientLogRoot();
  const relative = clientLogStoragePath(userId, id, filename);
  const full = path.join(root, relative);
  await fs.mkdir(path.dirname(full), { recursive: true });
  await fs.writeFile(full, bytes);
  return relative;
}

export async function readClientLogFile(storagePath: string): Promise<Buffer> {
  const full = path.join(clientLogRoot(), storagePath);
  return fs.readFile(full);
}

export async function deleteClientLogFile(storagePath: string): Promise<void> {
  const full = path.join(clientLogRoot(), storagePath);
  await fs.unlink(full).catch(() => undefined);
}
