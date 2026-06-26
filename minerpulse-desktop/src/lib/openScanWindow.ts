import { invoke } from "@tauri-apps/api/core";

export async function openScanWindow(): Promise<void> {
  await invoke("open_scan_window");
}
