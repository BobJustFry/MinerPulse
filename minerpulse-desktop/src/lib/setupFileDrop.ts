import { getCurrentWebview } from "@tauri-apps/api/webview";

function isMinerDropPath(path: string): boolean {
  const name = path.replace(/\\/g, "/").split("/").pop()?.toLowerCase() ?? "";
  return (
    name.endsWith(".mpsn") ||
    name.endsWith(".mprs") ||
    name.endsWith(".mpulse") ||
    name.endsWith(".mpulse-snap") ||
    name.endsWith(".mpulse-session") ||
    name.endsWith(".txt") ||
    name.endsWith(".log") ||
    name.endsWith(".json")
  );
}

/**
 * Desktop file drop via Tauri paths (supports binary .mpsn/.mprs).
 * Do not read dropped files as UTF-8 text — binary snapshots are MPSN/MPRS.
 */
export function setupFileDrop(handlers: {
  onHover: () => void;
  onLeave: () => void;
  onDropPath: (path: string) => void | Promise<void>;
  onError: (err: unknown) => void;
  onUnsupported: () => void;
}): () => void {
  let disposed = false;
  let unlisten: (() => void) | undefined;
  let hoverDepth = 0;

  void getCurrentWebview()
    .onDragDropEvent((event) => {
      const payload = event.payload;
      if (payload.type === "enter") {
        hoverDepth += 1;
        if (hoverDepth === 1) handlers.onHover();
        return;
      }
      if (payload.type === "over") {
        if (hoverDepth === 0) {
          hoverDepth = 1;
          handlers.onHover();
        }
        return;
      }
      if (payload.type === "leave") {
        hoverDepth = 0;
        handlers.onLeave();
        return;
      }
      if (payload.type === "drop") {
        hoverDepth = 0;
        handlers.onLeave();
        const path = payload.paths[0];
        if (!path) {
          handlers.onUnsupported();
          return;
        }
        if (!isMinerDropPath(path)) {
          handlers.onUnsupported();
          return;
        }
        void Promise.resolve(handlers.onDropPath(path)).catch((err) => handlers.onError(err));
      }
    })
    .then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      unlisten = fn;
    })
    .catch((err) => handlers.onError(err));

  return () => {
    disposed = true;
    unlisten?.();
    unlisten = undefined;
  };
}
