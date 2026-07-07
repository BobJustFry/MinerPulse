import { invoke } from "@tauri-apps/api/core";

export const MINER_READ_TIMEOUT_MS = 20_000;

export async function invokeWithTimeout<T>(
  cmd: string,
  args: Record<string, unknown>,
  timeoutMs: number,
  onTimeout?: () => void | Promise<void>,
): Promise<T> {
  let timedOut = false;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const timeoutPromise = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      timedOut = true;
      void onTimeout?.();
      reject({ code: "CONN_TIMEOUT", args: null });
    }, timeoutMs);
  });

  try {
    return await Promise.race([invoke<T>(cmd, args), timeoutPromise]);
  } catch (err) {
    if (timedOut) {
      throw { code: "CONN_TIMEOUT", args: null };
    }
    throw err;
  } finally {
    if (timer !== undefined) {
      clearTimeout(timer);
    }
  }
}
