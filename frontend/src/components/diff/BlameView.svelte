<script lang="ts">
  import { formatCommitDate, shortOid } from "$lib/format";
  import { visibleRange } from "$lib/graph-geometry";
  import type { BlameLine } from "$lib/ipc";

  interface Props {
    lines: readonly BlameLine[];
    path: string;
    onselect: (oid: string) => void;
  }

  let { lines, path, onselect }: Props = $props();

  const ROW_HEIGHT = 18;
  const BUFFER_ROWS = 12;

  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewportHeight = $state(0);

  const range = $derived(
    visibleRange(scrollTop, viewportHeight, ROW_HEIGHT, lines.length, BUFFER_ROWS),
  );
  const visible = $derived(
    lines.slice(range.start, range.end).map((line, i) => ({ line, at: range.start + i })),
  );

  /** Only the first line of a run shows the annotation, as `git blame` does. */
  function startsRun(at: number): boolean {
    return at === 0 || lines[at - 1]?.oid !== lines[at]?.oid;
  }

  $effect(() => {
    if (!scroller) return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry) viewportHeight = entry.contentRect.height;
    });
    observer.observe(scroller);
    return () => observer.disconnect();
  });
</script>

<div class="blame">
  <div class="bar">
    <span class="path mono truncate">{path}</span>
    <span class="count tabular">{lines.length} lines</span>
  </div>

  <div
    class="scroll"
    bind:this={scroller}
    onscroll={() => scroller && (scrollTop = scroller.scrollTop)}
  >
    <div class="rows" style:height="{lines.length * ROW_HEIGHT}px">
      {#each visible as item (item.at)}
        <div class="line" style:top="{item.at * ROW_HEIGHT}px">
          {#if startsRun(item.at)}
            <span
              class="annotation truncate"
              role="button"
              tabindex="-1"
              title="{item.line.summary} — {item.line.author}"
              onclick={() => onselect(item.line.oid)}
              onkeydown={(e) => e.key === "Enter" && onselect(item.line.oid)}
            >
              <span class="oid mono">{shortOid(item.line.oid)}</span>
              <span class="author truncate">{item.line.author}</span>
              <span class="date tabular">{formatCommitDate(item.line.timestamp, 0)}</span>
            </span>
          {:else}
            <span class="annotation"></span>
          {/if}
          <span class="num tabular">{item.line.line}</span>
          <span class="code mono">{item.line.text}</span>
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .blame {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 0 auto;
    padding: var(--sp-3) var(--sp-4);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-dense);
  }

  .path {
    flex: 1 1 auto;
    min-width: 0;
  }

  .count {
    color: var(--text-secondary);
    font-size: 11px;
  }

  .scroll {
    position: relative;
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  .rows {
    position: relative;
  }

  .line {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    height: 18px;
    font-size: var(--fs-code);
    white-space: pre;
  }

  .annotation {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 0 auto;
    width: 240px;
    padding: 0 var(--sp-4);
    color: var(--text-secondary);
    font-size: 10px;
    border-right: 1px solid var(--divider);
    cursor: default;
  }

  .annotation:hover {
    color: var(--status-ref);
  }

  .oid {
    flex: 0 0 auto;
  }

  .author {
    flex: 1 1 auto;
    min-width: 0;
  }

  .date {
    flex: 0 0 auto;
  }

  .num {
    flex: 0 0 auto;
    width: 48px;
    padding-right: var(--sp-3);
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: 10px;
    text-align: right;
  }

  .code {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
