import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Theme } from "$lib/types";

export function formatAppVersion(product: string, version: string, build: number): string {
  return `${product} ${version} (${build})`;
}

export function formatWindowTitle(product: string, version: string, build: number): string {
  return formatAppVersion(product, version, build);
}

export async function syncWindowChrome(options: {
  title: string;
  theme: Theme;
}): Promise<void> {
  const window = getCurrentWindow();
  await window.setTitle(options.title);
  await window.setTheme(options.theme === "dark" ? "dark" : "light");
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("sync_window_frame", { theme: options.theme });
}
