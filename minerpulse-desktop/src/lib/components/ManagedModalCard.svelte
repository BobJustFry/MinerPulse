<script lang="ts">
  import type { Snippet } from "svelte";
  import { loadModalLayout, readModalSize, saveModalLayout } from "$lib/modalLayout";

  let {
    layoutId,
    class: className = "",
    dragDisabled = false,
    resizable = true,
    defaultWidth = 560,
    defaultHeight,
    minWidth = 320,
    minHeight = 240,
    children,
    ...rest
  }: {
    layoutId: string;
    class?: string;
    dragDisabled?: boolean;
    resizable?: boolean;
    defaultWidth?: number;
    defaultHeight?: number;
    minWidth?: number;
    minHeight?: number;
    children: Snippet;
    [key: string]: unknown;
  } = $props();

  let cardEl = $state<HTMLDivElement | null>(null);
  let offset = $state({ x: 0, y: 0 });
  let cardWidth = $state<number | undefined>(undefined);
  let cardHeight = $state<number | undefined>(undefined);
  let dragState = $state<{
    pointerId: number;
    startX: number;
    startY: number;
    originX: number;
    originY: number;
  } | null>(null);
  let resizeObserver: ResizeObserver | undefined;
  let resizeTimer: ReturnType<typeof setTimeout> | undefined;
  let headCleanup: (() => void) | undefined;

  function persistLayout() {
    if (!cardEl) return;
    const size = readModalSize(cardEl);
    saveModalLayout(layoutId, {
      x: offset.x,
      y: offset.y,
      width: size.width,
      height: size.height,
    });
  }

  function schedulePersistSize() {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(() => {
      persistLayout();
    }, 250);
  }

  function bindDragHandle(head: HTMLElement) {
    headCleanup?.();
    const onPointerDown = (event: PointerEvent) => {
      if (dragDisabled || event.button !== 0) return;
      if ((event.target as HTMLElement).closest(".modal-close")) return;
      dragState = {
        pointerId: event.pointerId,
        startX: event.clientX,
        startY: event.clientY,
        originX: offset.x,
        originY: offset.y,
      };
      head.setPointerCapture(event.pointerId);
      event.preventDefault();
    };
    const onPointerMove = (event: PointerEvent) => {
      if (!dragState || dragState.pointerId !== event.pointerId) return;
      offset = {
        x: dragState.originX + (event.clientX - dragState.startX),
        y: dragState.originY + (event.clientY - dragState.startY),
      };
    };
    const onPointerUp = (event: PointerEvent) => {
      if (!dragState || dragState.pointerId !== event.pointerId) return;
      dragState = null;
      persistLayout();
      try {
        head.releasePointerCapture(event.pointerId);
      } catch {
        /* ignore */
      }
    };
    head.addEventListener("pointerdown", onPointerDown);
    head.addEventListener("pointermove", onPointerMove);
    head.addEventListener("pointerup", onPointerUp);
    head.addEventListener("pointercancel", onPointerUp);
    head.classList.add("managed-modal-drag-head");
    headCleanup = () => {
      head.removeEventListener("pointerdown", onPointerDown);
      head.removeEventListener("pointermove", onPointerMove);
      head.removeEventListener("pointerup", onPointerUp);
      head.removeEventListener("pointercancel", onPointerUp);
      head.classList.remove("managed-modal-drag-head");
    };
  }

  $effect(() => {
    layoutId;
    defaultWidth;
    defaultHeight;
    const saved = loadModalLayout(layoutId);
    offset = { x: saved?.x ?? 0, y: saved?.y ?? 0 };
    cardWidth = saved?.width ?? defaultWidth;
    cardHeight = saved?.height ?? defaultHeight;
  });

  $effect(() => {
    if (!cardEl) return;
    const head = cardEl.querySelector<HTMLElement>(".modal-head");
    if (!head) return;
    bindDragHandle(head);
    return () => {
      headCleanup?.();
      headCleanup = undefined;
    };
  });

  $effect(() => {
    if (!cardEl || !resizable) return;
    resizeObserver = new ResizeObserver(() => {
      schedulePersistSize();
    });
    resizeObserver.observe(cardEl);
    return () => {
      resizeObserver?.disconnect();
      resizeObserver = undefined;
      clearTimeout(resizeTimer);
    };
  });
</script>

<div
  bind:this={cardEl}
  class="modal-card managed-modal-card {className}"
  class:is-dragging={dragState != null}
  class:is-resizable={resizable}
  style:transform="translate({offset.x}px, {offset.y}px)"
  style:width={cardWidth ? `${cardWidth}px` : undefined}
  style:height={cardHeight ? `${cardHeight}px` : undefined}
  style:min-width="{minWidth}px"
  style:min-height="{minHeight}px"
  {...rest}
>
  {@render children()}
  {#if resizable}
    <div class="managed-modal-resize-grip" aria-hidden="true"></div>
  {/if}
</div>

<style>
  .managed-modal-card {
    position: relative;
    flex-shrink: 0;
    will-change: transform;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    max-width: min(96vw, 920px);
    max-height: calc(100vh - 32px);
  }

  .managed-modal-card.is-resizable {
    resize: both;
  }

  .managed-modal-card.is-dragging {
    user-select: none;
  }

  .managed-modal-card :global(.managed-modal-drag-head) {
    cursor: grab;
    user-select: none;
    touch-action: none;
  }

  .managed-modal-card.is-dragging :global(.managed-modal-drag-head) {
    cursor: grabbing;
  }

  .managed-modal-resize-grip {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 18px;
    height: 18px;
    cursor: nwse-resize;
    opacity: 0.35;
    background: linear-gradient(
      135deg,
      transparent 0 45%,
      var(--border-strong) 45% 55%,
      transparent 55% 70%,
      var(--border-strong) 70% 80%,
      transparent 80%
    );
  }
</style>
