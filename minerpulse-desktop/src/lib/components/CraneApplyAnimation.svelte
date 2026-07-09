<script lang="ts">
  let {
    active = false,
    status = "idle",
  }: {
    active?: boolean;
    status?: "idle" | "loading" | "success" | "error";
  } = $props();
</script>

<div class="crane-anim" class:active class:success={status === "success"} class:error={status === "error"}>
  <svg viewBox="0 0 120 72" aria-hidden="true">
    <rect class="crane-mast" x="8" y="8" width="6" height="56" rx="2" />
    <rect class="crane-arm" x="8" y="12" width="72" height="5" rx="2" />
    <line class="crane-cable" x1="72" y1="17" x2="72" y2="42" />
    <rect class="crane-block block-a" x="64" y="42" width="16" height="10" rx="2" />
    <rect class="crane-block block-b" x="84" y="52" width="16" height="10" rx="2" />
    <rect class="crane-block block-c" x="64" y="52" width="16" height="10" rx="2" />
  </svg>
</div>

<style>
  .crane-anim {
    width: 120px;
    height: 72px;
    opacity: 0.35;
    transition: opacity 0.2s ease;
  }

  .crane-anim.active {
    opacity: 1;
  }

  .crane-anim svg {
    width: 100%;
    height: 100%;
    fill: var(--accent);
  }

  .crane-mast,
  .crane-arm,
  .crane-cable {
    fill: var(--text-muted);
    stroke: none;
  }

  .crane-cable {
    stroke: var(--text-muted);
    stroke-width: 2;
  }

  .crane-block {
    fill: var(--accent);
    transform-origin: 72px 42px;
  }

  .crane-anim.active .crane-block {
    animation: lift 1.2s ease-in-out infinite alternate;
  }

  .crane-anim.success .crane-block {
    animation: settle 0.5s ease forwards;
  }

  .crane-anim.error {
    animation: shake 0.35s ease;
  }

  @keyframes lift {
    from {
      transform: translateY(0);
    }
    to {
      transform: translateY(-10px);
    }
  }

  @keyframes settle {
    to {
      transform: translateY(0);
    }
  }

  @keyframes shake {
    0%,
    100% {
      transform: translateX(0);
    }
    25% {
      transform: translateX(-4px);
    }
    75% {
      transform: translateX(4px);
    }
  }
</style>
