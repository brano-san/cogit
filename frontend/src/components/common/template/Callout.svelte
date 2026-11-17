<script lang="ts">
  import Icon from "$components/common/Icon.svelte";
  import type { Snippet } from "svelte";

  /** Warning strip: icon + text, 3 px bar on the left. `warning` for a risk, `danger` for
      data that will be lost. */
  interface Props {
    kind?: "warning" | "danger";
    children: Snippet;
  }

  let { kind = "warning", children }: Props = $props();
</script>

<div class="callout {kind}" role="note">
  <Icon name={kind === "danger" ? "error" : "warning"} />
  <div class="text">{@render children()}</div>
</div>

<style>
  .callout {
    --tone: var(--status-warning);
    display: flex;
    gap: var(--sp-4);
    align-items: flex-start;
    padding: var(--sp-4) var(--sp-5);
    background: var(--badge-warning-bg);
    border-left: 3px solid var(--tone);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    line-height: 1.4;
  }

  .callout.danger {
    --tone: var(--status-danger);
    background: color-mix(in srgb, var(--status-danger) 16%, var(--bg-elevated));
  }

  .callout :global(svg) {
    flex: 0 0 auto;
    width: 14px;
    height: 14px;
    margin-top: 1px;
    fill: none;
    stroke: var(--tone);
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .text {
    min-width: 0;
    color: var(--text-primary);
    overflow-wrap: anywhere;
  }
</style>
