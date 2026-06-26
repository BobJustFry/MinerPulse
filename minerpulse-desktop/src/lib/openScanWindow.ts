import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

const SCAN_LABEL = "scan";

export async function openScanWindow(): Promise<void> {
  const existing = await WebviewWindow.getByLabel(SCAN_LABEL);
  if (existing) {
    await existing.show();
    await existing.setFocus();
    return;
  }

  new WebviewWindow(SCAN_LABEL, {
    url: "/scan",
    title: "MinerPulse — Scan",
    width: 540,
    height: 680,
    minWidth: 420,
    minHeight: 480,
    resizable: true,
    center: true,
  });
}
