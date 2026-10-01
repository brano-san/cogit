<script lang="ts">
  import type { IconState } from "$lib/file-state";

  /** The 16x16 icon of a file or a repository in a state: the base on the 1 px grid (strokes on
      half pixels, so they stay one sharp pixel at 100 and 200 % scale) and an 8x8 overlay in the
      bottom-right corner. Every color is a `--file-icon-*` token. `overlayOnly` draws just the
      overlay, to lay it over another icon. */
  interface Props {
    state: IconState;
    base?: "page" | "repository";
    overlayOnly?: boolean;
    label?: string;
  }

  let { state, base = "page", overlayOnly = false, label }: Props = $props();
</script>

<svg
  class="icon {state}"
  class:overlay-only={overlayOnly}
  viewBox="0 0 16 16"
  role="img"
  aria-label={label}
  aria-hidden={label ? undefined : true}
  shape-rendering="crispEdges"
>
  {#if !overlayOnly}
    <g class="base">
      {#if base === "repository"}
        <path d="M3.5 1.5H12.5V14.5H3.5Z" />
        <path class="line" d="M3.5 11.5H12.5M6.5 4.5H9.5" />
      {:else}
        <path d="M3.5 1.5H9.5L12.5 4.5V14.5H3.5Z" />
        <path class="line" d="M9.5 1.5V4.5H12.5" />
      {/if}
    </g>
  {/if}
  <g transform="translate(8 8)" class="over">
    {#if state === "untracked"}
      <rect width="8" height="8" rx="1" fill="var(--file-icon-overlay-added)" />
      <path d="M3 2H5V3H6V5H5V6H3V5H2V3H3Z" fill="var(--file-icon-overlay-glyph)" />
    {:else if state === "added"}
      <path d="M3 0H5V3H8V5H5V8H3V5H0V3H3Z" fill="var(--file-icon-overlay-added)" />
    {:else if state === "staged"}
      <path
        d="M1 4.5L3.25 6.75L7 1.5"
        fill="none"
        stroke="var(--file-icon-overlay-staged)"
        stroke-width="2"
        stroke-linecap="square"
        shape-rendering="geometricPrecision"
      />
    {:else if state === "removed"}
      <rect y="3" width="8" height="2" fill="var(--file-icon-overlay-removed)" />
    {:else if state === "renamed"}
      <path d="M0 3H4V0.5L8 4L4 7.5V5H0Z" fill="var(--file-icon-overlay-renamed)" shape-rendering="geometricPrecision" />
    {:else if state === "conflicted"}
      <circle cx="4" cy="4" r="4" fill="var(--file-icon-page)" />
      <circle cx="4" cy="4" r="3" fill="var(--file-icon-overlay-removed)" shape-rendering="geometricPrecision" />
      <path d="M3 1.5H5V4H3ZM3 5H5V6.5H3Z" fill="var(--file-icon-overlay-glyph)" />
    {/if}
  </g>
</svg>

<style>
  .icon {
    flex: none;
    width: var(--file-icon);
    height: var(--file-icon);
    --fill: var(--file-icon-page);
    --line: var(--file-icon-stroke);
  }

  /* Laid over a --kind-icon box, whose center it shares. */
  .icon.overlay-only {
    position: absolute;
    top: calc((var(--kind-icon) - var(--file-icon)) / 2);
    left: calc((var(--kind-icon) - var(--file-icon)) / 2);
    pointer-events: none;
  }

  .base path {
    fill: var(--fill);
    stroke: var(--line);
    stroke-width: 1;
    stroke-linejoin: miter;
  }

  .base .line {
    fill: none;
  }

  .modified {
    --fill: var(--file-icon-modified-fill);
    --line: var(--file-icon-modified-stroke);
  }

  .removed {
    --fill: var(--file-icon-modified-fill);
  }

  .conflicted {
    --fill: var(--file-icon-conflict-fill);
    --line: var(--file-icon-modified-stroke);
  }

  .missing {
    --fill: none;
  }

  .missing .base path {
    stroke-dasharray: 2 1;
  }

  .ignored {
    --line: var(--fg-disabled);
    opacity: 0.5;
  }
</style>
