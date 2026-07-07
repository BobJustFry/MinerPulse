import { invoke } from "@tauri-apps/api/core";
import {
  isImportCandidate,
  type ParseImportResponse,
} from "$lib/importFile";

function hasFiles(dataTransfer: DataTransfer | null): boolean {
  if (!dataTransfer) return false;
  return Array.from(dataTransfer.types).includes("Files");
}

export function setupFileDrop(handlers: {
  onHover: () => void;
  onLeave: () => void;
  onDrop: (result: ParseImportResponse) => void | Promise<void>;
  onError: (err: unknown) => void;
  onTooLarge: () => void;
}): () => void {
  let dragDepth = 0;

  const resetDrag = () => {
    dragDepth = 0;
    handlers.onLeave();
  };

  const onDragEnter = (event: DragEvent) => {
    event.preventDefault();
    if (!hasFiles(event.dataTransfer)) return;
    dragDepth += 1;
    if (dragDepth === 1) {
      handlers.onHover();
    }
  };

  const onDragOver = (event: DragEvent) => {
    event.preventDefault();
    if (hasFiles(event.dataTransfer)) {
      event.dataTransfer!.dropEffect = "copy";
    }
  };

  const onDragLeave = (event: DragEvent) => {
    event.preventDefault();
    const related = event.relatedTarget as Node | null;
    if (related && document.documentElement.contains(related)) {
      return;
    }
    dragDepth = Math.max(0, dragDepth - 1);
    if (dragDepth === 0) {
      handlers.onLeave();
    }
  };

  const onDrop = async (event: DragEvent) => {
    event.preventDefault();
    dragDepth = 0;
    handlers.onLeave();

    const file = event.dataTransfer?.files.item(0);
    if (!isImportCandidate(file)) {
      handlers.onTooLarge();
      return;
    }

    try {
      const content = await file.text();
      const result = await invoke<ParseImportResponse>("parse_import_file", {
        content,
        filename: file.name,
      });
      await handlers.onDrop(result);
    } catch (err) {
      handlers.onError(err);
    }
  };

  window.addEventListener("dragenter", onDragEnter, true);
  window.addEventListener("dragover", onDragOver, true);
  window.addEventListener("dragleave", onDragLeave, true);
  window.addEventListener("drop", onDrop, true);
  window.addEventListener("dragend", resetDrag, true);
  const onVisibilityChange = () => {
    if (document.visibilityState !== "visible") {
      resetDrag();
    }
  };
  window.addEventListener("blur", resetDrag);
  document.addEventListener("visibilitychange", onVisibilityChange);

  return () => {
    window.removeEventListener("dragenter", onDragEnter, true);
    window.removeEventListener("dragover", onDragOver, true);
    window.removeEventListener("dragleave", onDragLeave, true);
    window.removeEventListener("drop", onDrop, true);
    window.removeEventListener("dragend", resetDrag, true);
    window.removeEventListener("blur", resetDrag);
    document.removeEventListener("visibilitychange", onVisibilityChange);
  };
}
