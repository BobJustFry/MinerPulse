import { invoke } from "@tauri-apps/api/core";

export const MINER_READ_TIMEOUT_MS = 10_000;

export async function invokeWithTimeout<T>(
  cmd: string,
  args: Record<string, unknown>,
  timeoutMs: number,
  onTimeout?: () => void | Promise<void>,
): Promise<T> {
  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    void onTimeout?.();
  }, timeoutMs);

  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    if (timedOut) {
      throw { code: "CONN_TIMEOUT", args: null };
    }
    throw err;
  } finally {
    clearTimeout(timer);
  }
}
