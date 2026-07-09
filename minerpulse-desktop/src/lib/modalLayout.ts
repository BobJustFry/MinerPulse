export interface ModalLayout {
  x: number;
  y: number;
  width?: number;
  height?: number;
}

const STORAGE_KEY = "minerpulse.modalLayouts";

function readAll(): Record<string, ModalLayout> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Record<string, ModalLayout>;
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch {
    return {};
  }
}

export function loadModalLayout(id: string): ModalLayout | null {
  const item = readAll()[id];
  if (!item || typeof item.x !== "number" || typeof item.y !== "number") {
    return null;
  }
  return item;
}

export function saveModalLayout(id: string, layout: ModalLayout): void {
  try {
    const all = readAll();
    all[id] = layout;
    localStorage.setItem(STORAGE_KEY, JSON.stringify(all));
  } catch {
    /* ignore quota / private mode */
  }
}

export function readModalSize(card: HTMLElement): Pick<ModalLayout, "width" | "height"> {
  return {
    width: card.offsetWidth,
    height: card.offsetHeight,
  };
}
