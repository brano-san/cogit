<script lang="ts">
  import { ACTION_LEFT_X, ACTION_RIGHT_X, BAND_WIDTH } from "$lib/diff-band";
  import type { BandConnector } from "$lib/solver-geometry";

  /** The strip between two panes: a ribbon for every hunk near the viewport, and over it
      the button that takes one side into the Result. */
  interface Props {
    connectors: BandConnector[];
    /** How the pane next to the button colors that hunk. */
    tone: (id: number) => "add" | "del" | "changed" | null;
    current: number | null;
    /** Undecided conflicts: their ribbon has the outline of danger. */
    open: ReadonlySet<number>;
    /** Which side the buttons take: `»` takes Ours, `«` takes Theirs; none on a band
        between two sides. */
    button: "ours" | "theirs" | null;
    show: (id: number) => boolean;
    onact: (id: number) => void;
    label: string;
  }

  let { connectors, tone, current, open, button, show, onact, label }: Props = $props();
</script>

<div class="band" style:width="{BAND_WIDTH}px">
  <svg class="ribbons" width={BAND_WIDTH} height="100%" aria-hidden="true">
    {#each connectors as c (c.id)}
      <path class="fill {tone(c.id) ?? 'plain'}" d={c.path}></path>
    {/each}
    {#each connectors as c (c.id)}
      <path
        class="edge {tone(c.id) ?? 'plain'}"
        class:open={open.has(c.id)}
        class:current={current === c.id}
        d={c.edges}
      ></path>
    {/each}
  </svg>
  {#if button}
    <div class="ui">
      {#each connectors as c (c.id)}
        {@const y = button === "ours" ? c.anchor.left : c.anchor.right}
        {#if y !== null && show(c.id)}
          <button
            type="button"
            class="act"
            style:left="{button === 'ours' ? ACTION_LEFT_X : ACTION_RIGHT_X}px"
            style:top="{y}px"
            title={label}
            aria-label={label}
            onclick={() => onact(c.id)}>{button === "ours" ? "»" : "«"}</button
          >
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .band {
    position: relative;
    flex: 0 0 auto;
    height: 100%;
    overflow: hidden;
    background: var(--diff-center-gutter-bg);
    user-select: none;
  }

  .ribbons {
    position: absolute;
    top: 0;
    left: 0;
    pointer-events: none;
  }

  .fill {
    fill: var(--diff-connector-fill);
    stroke: none;
  }

  .edge {
    fill: none;
    stroke: var(--diff-connector-stroke);
    stroke-width: 1;
    stroke-linecap: butt;
    shape-rendering: geometricPrecision;
  }

  .fill.changed {
    fill: var(--diff-changed-line);
  }

  .edge.changed {
    stroke: var(--diff-changed-word);
  }

  .fill.add {
    fill: var(--diff-add-line);
  }

  .edge.add {
    stroke: var(--diff-add-word);
  }

  .fill.del {
    fill: var(--diff-del-line);
  }

  .edge.del {
    stroke: var(--diff-del-word);
  }

  .edge.open {
    stroke: var(--status-danger);
  }

  .edge.current {
    stroke-width: 2;
  }

  .ui {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }

  .act {
    position: absolute;
    transform: translate(-50%, -50%);
    width: 14px;
    height: 18px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--diff-center-gutter-action);
    font-size: 13px;
    font-weight: 700;
    line-height: 18px;
    text-align: center;
    pointer-events: auto;
    cursor: pointer;
  }

  .act:hover {
    color: var(--diff-center-gutter-action-hover);
  }
</style>
