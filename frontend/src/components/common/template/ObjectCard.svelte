<script module lang="ts">
  /** One row of the card. */
  export interface CardRow {
    label: string;
    value: string;
    /** Monospace in `fg.muted`: refs, hashes, paths. Plain text is `fg.primary`. */
    mono?: boolean;
    /** Maximum lines; the rest is cut with `…` and shown in a tooltip. */
    clamp?: number;
  }
</script>

<script lang="ts">
  import { isClamped } from "$lib/dialog-template";

  /** Key/value card of the object a dialog acts on (stash, worktree, ref). `bg.panel`
      surface; values wrap and are never cut by the width. A row with `clamp` ends in `…`
      after that many lines, with the full text in the tooltip. */
  interface Props {
    rows: readonly CardRow[];
  }

  let { rows }: Props = $props();

  /** The tooltip only exists while the text is actually cut. */
  function clampTip(node: HTMLElement, value: string) {
    const update = (text: string) => {
      if (isClamped(node.scrollHeight, node.clientHeight)) node.title = text;
      else node.removeAttribute("title");
    };
    update(value);
    return { update };
  }
</script>

<dl class="card">
  {#each rows as row (row.label)}
    <dt>{row.label}</dt>
    <dd class:mono={row.mono} class:clamped={row.clamp} style:--lines={row.clamp} use:clampTip={row.value}>
      {row.value}
    </dd>
  {/each}
</dl>

<style>
  .card {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: var(--sp-3) var(--sp-5);
    margin: 0;
    padding: var(--sp-5);
    background: var(--bg-panel);
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    line-height: 1.4;
  }

  dt {
    color: var(--text-secondary);
  }

  dd {
    margin: 0;
    min-width: 0;
    color: var(--text-primary);
    white-space: pre-line;
    overflow-wrap: anywhere;
    user-select: text;
  }

  dd.mono {
    font-family: var(--font-mono);
    color: var(--text-muted);
  }

  dd.clamped {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: var(--lines);
    line-clamp: var(--lines);
    overflow: hidden;
  }
</style>
