import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { ParseImportResponse } from "$lib/importFile";

export async function setupNativeFileDrop(handlers: {
  onHover: () => void;
  onLeave: () => void;
  onDrop: (result: ParseImportResponse, fileName: string) => void;
  onError: (message: string) => void;
  onTooLarge: () => void;
}): Promise<() => void> {
  const webview = getCurrentWebview();

  const unlisten = await webview.onDragDropEvent(async (event) => {
    const payload = event.payload;

    if (payload.type === "enter" || payload.type === "over") {
      handlers.onHover();
      return;
    }

    if (payload.type === "leave") {
      handlers.onLeave();
      return;
    }

    if (payload.type !== "drop") {
      handlers.onLeave();
      return;
    }

    handlers.onLeave();

    const path = payload.paths[0];
    if (!path) return;

    const fileName = path.split(/[/\\]/).pop() ?? path;

    try {
      const result = await invoke<ParseImportResponse>("import_file_path", { path });
      handlers.onDrop(result, fileName);
    } catch (err) {
      handlers.onError(String(err));
    }
  });

  return unlisten;
}
