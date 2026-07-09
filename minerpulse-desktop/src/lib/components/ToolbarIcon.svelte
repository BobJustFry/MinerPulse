<script lang="ts">
  export type ToolbarIconName =
    | "scan"
    | "stop"
    | "cancel"
    | "read"
    | "poll"
    | "record"
    | "open"
    | "save"
    | "theme-dark"
    | "theme-light"
    | "density-compact"
    | "density-comfortable"
    | "about"
    | "play"
    | "pause"
    | "rewind"
    | "close";

  let { name }: { name: ToolbarIconName } = $props();
</script>

<svg class="toolbar-icon" viewBox="0 0 16 16" aria-hidden="true">
  {#if name === "scan"}
    <circle cx="7" cy="7" r="4.25" fill="none" stroke="currentColor" stroke-width="1.4" />
    <path d="M10.5 10.5 13.5 13.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
  {:else if name === "stop"}
    <rect x="4.5" y="4.5" width="7" height="7" rx="1" fill="currentColor" />
  {:else if name === "cancel"}
    <path d="m5 5 6 6M11 5 5 11" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
  {:else if name === "read"}
    <path
      d="M8 2.5v7M5.5 7 8 9.5 10.5 7M4 12.5h8"
      fill="none"
      stroke="currentColor"
      stroke-width="1.4"
      stroke-linecap="round"
      stroke-linejoin="round"
    />
  {:else if name === "poll"}
    <path
      d="M11.5 5.5A4.25 4.25 0 1 0 8 12.25V14M8 3.75V2"
      fill="none"
      stroke="currentColor"
      stroke-width="1.4"
      stroke-linecap="round"
      stroke-linejoin="round"
    />
  {:else if name === "record"}
    <circle cx="8" cy="8" r="3.25" fill="currentColor" />
  {:else if name === "open"}
    <path
      d="M3 5.5V12a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V6.5M2.5 5.5h5l1.25-1.5H12a1 1 0 0 1 1 1v1"
      fill="none"
      stroke="currentColor"
      stroke-width="1.2"
      stroke-linecap="round"
      stroke-linejoin="round"
    />
  {:else if name === "save"}
    <path
      d="M4 3.5h6l2 2V12.5H4zM6 3.5v3h4V3.5M6.5 10h3"
      fill="none"
      stroke="currentColor"
      stroke-width="1.2"
      stroke-linecap="round"
      stroke-linejoin="round"
    />
  {:else if name === "theme-dark"}
    <path
      d="M9.25 2.75a4.25 4.25 0 1 0 3 7.25A5.25 5.25 0 1 1 9.25 2.75Z"
      fill="none"
      stroke="currentColor"
      stroke-width="1.3"
      stroke-linejoin="round"
    />
  {:else if name === "theme-light"}
    <circle cx="8" cy="8" r="2.75" fill="none" stroke="currentColor" stroke-width="1.3" />
    <path
      d="M8 2v1.25M8 12.75V14M2 8h1.25M12.75 8H14M3.8 3.8l.9.9M11.3 11.3l.9.9M12.2 3.8l-.9.9M4.7 11.3l-.9.9"
      stroke="currentColor"
      stroke-width="1.2"
      stroke-linecap="round"
    />
  {:else if name === "density-compact"}
    <path
      d="M5.5 3.5 3.5 5.5M10.5 3.5l2 2M5.5 12.5 3.5 10.5M10.5 12.5l2-2"
      stroke="currentColor"
      stroke-width="1.4"
      stroke-linecap="round"
    />
  {:else if name === "density-comfortable"}
    <path
      d="M3.5 5.5 5.5 3.5M12.5 5.5l-2-2M3.5 10.5 5.5 12.5M12.5 10.5l-2 2"
      stroke="currentColor"
      stroke-width="1.4"
      stroke-linecap="round"
    />
  {:else if name === "about"}
    <circle cx="8" cy="8" r="5.25" fill="none" stroke="currentColor" stroke-width="1.3" />
    <path d="M8 7.1V11M8 5.25h.01" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
  {:else if name === "play"}
    <path d="M6 4.5 11.5 8 6 11.5z" fill="currentColor" />
  {:else if name === "pause"}
    <path d="M5.5 4.5h2v7h-2zM8.5 4.5h2v7h-2z" fill="currentColor" />
  {:else if name === "rewind"}
    <path
      d="M4 5.5v5M7.5 5.5 4 8l3.5 2.5M11 5.5v5"
      fill="none"
      stroke="currentColor"
      stroke-width="1.3"
      stroke-linecap="round"
      stroke-linejoin="round"
    />
  {:else if name === "close"}
    <path d="m5 5 6 6M11 5 5 11" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
  {/if}
</svg>
