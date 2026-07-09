<script lang="ts">
  import ToolbarIcon, { type ToolbarIconName } from "$lib/components/ToolbarIcon.svelte";
  import type { Density } from "$lib/types";
  import type { Snippet } from "svelte";

  let {
    label,
    icon,
    density = "comfortable",
    title,
    class: className = "",
    disabled = false,
    type = "button",
    onclick,
    children,
    ...rest
  }: {
    label: string;
    icon: ToolbarIconName;
    density?: Density;
    title?: string;
    class?: string;
    disabled?: boolean;
    type?: "button" | "submit" | "reset";
    onclick?: (event: MouseEvent) => void;
    children?: Snippet;
    [key: string]: unknown;
  } = $props();

  const tooltip = $derived(title ?? label);
  const iconOnly = $derived(density === "compact");
</script>

<button
  {type}
  class="btn {className}"
  class:btn-icon-only={iconOnly}
  class:btn-with-spinner={!!children}
  title={tooltip}
  aria-label={iconOnly ? label : undefined}
  {disabled}
  {onclick}
  {...rest}
>
  {#if children}
    {@render children()}
  {/if}
  {#if iconOnly}
    <ToolbarIcon name={icon} />
  {:else}
    {label}
  {/if}
</button>
