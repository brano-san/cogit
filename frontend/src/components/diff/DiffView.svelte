<script lang="ts">
  import { flatten, pairRows, segments, type FlatEntry, type SideCell } from "$lib/diff-rows";
  import { visibleRange } from "$lib/graph-geometry";
  import type { FileDiff, Hunk } from "$lib/ipc";

  interface Props {
    diff: FileDiff;
    path: string;
  }

  let { diff, path }: Props = $props();

  const ROW_HEIGHT = 18;
  const BUFFER_ROWS = 12;

  let mode = $state<"unified" | "split">("split");
  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  let current = $state(0);

  const hunks = $derived<Hunk[]>(diff.kind === "text" ? diff.hunks : []);
  const unified = $derived(flatten(hunks));
  const split = $derived.by(() => {
    const out: { hunk: number; header?: string; pair?: ReturnType<typeof pairRows>[number] }[] = [];
    hunks.forEach((hunk, index) => {
      out.push({ hunk: index, header: hunk.header });
      for (const pair of pairRows(hunk.rows)) out.push({ hunk: index, pair });
    });
    return out;
  });

  const total = $derived(mode === "unified" ? unified.length : split.length);
  const range = $derived(
    visibleRange(scrollTop, viewportHeight, ROW_HEIGHT, total, BUFFER_ROWS),
  );
  const headerOffsets = $derived.by(() => {
    const rows = mode === "unified" ? unified : split;
    const offsets: number[] = [];
    rows.forEach((row, index) => {
      const isHeader = mode === "unified" ? (row as FlatEntry).kind === "header" : "header" in row;
      if (isHeader) offsets.push(index);
    });
    return offsets;
  });

  function sign(cell: SideCell | null): string {
    if (!cell) return "";
    return cell.kind === "delete" ? "−" : cell.kind === "insert" ? "+" : " ";
  }

  function cells(cell: SideCell | null) {
    return cell ? segments(cell.text, cell.inline) : [];
  }

  function jump(delta: number) {
    if (headerOffsets.length === 0 || !scroller) return;
    current = Math.min(Math.max(current + delta, 0), headerOffsets.length - 1);
    scroller.scrollTop = (headerOffsets[current] ?? 0) * ROW_HEIGHT;
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key !== "F6") return;
    event.preventDefault();
    jump(event.shiftKey ? -1 : 1);
  }

  $effect(() => {
    if (!scroller) return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry) viewportHeight = entry.contentRect.height;
    });
    observer.observe(scroller);
    return () => observer.disconnect();
  });

  $effect(() => {
    void path;
    current = 0;
    if (scroller) scroller.scrollTop = 0;
  });
</script>

<svelte:window {onkeydown} />

<div class="diff">
  <div class="bar">
    <span class="path mono truncate">{path}</span>
    {#if diff.kind === "text"}
      <span class="eol">{diff.eol.old} → {diff.eol.new}</span>
      {#if diff.lossyEncoding}<span class="warn">not valid UTF-8</span>{/if}
      <button type="button" onclick={() => jump(-1)} title="Previous change (Shift+F6)">▲</button>
      <button type="button" onclick={() => jump(1)} title="Next change (F6)">▼</button>
      <button
        type="button"
        class="mode"
        onclick={() => (mode = mode === "split" ? "unified" : "split")}
      >
        {mode === "split" ? "Unified" : "Side by side"}
      </button>
    {/if}
  </div>

  {#if diff.kind === "unchanged"}
    <p class="message">No change in this file.</p>
  {:else if diff.kind === "eolOnly"}
    <p class="message">
      Only the line endings changed: {diff.from} → {diff.to}. The content is identical.
    </p>
  {:else if diff.kind === "binary"}
    <p class="message">Binary file — {diff.oldSize} bytes → {diff.newSize} bytes.</p>
  {:else if diff.kind === "image"}
    <p class="message">Image ({diff.mime}) — {diff.oldSize} bytes → {diff.newSize} bytes.</p>
  {:else if diff.kind === "tooLarge"}
    <p class="message">File is too large to diff ({diff.size} bytes).</p>
  {:else}
    <div class="scroll" bind:this={scroller} onscroll={() => scroller && (scrollTop = scroller.scrollTop)}>
      <div class="rows" style:height="{total * ROW_HEIGHT}px">
        {#if mode === "unified"}
          {#each unified.slice(range.start, range.end) as entry, index (range.start + index)}
            <div class="line" style:top="{(range.start + index) * ROW_HEIGHT}px">
              {#if entry.kind === "header"}
                <span class="header mono">{entry.text}</span>
              {:else if entry.row.kind === "context"}
                <span class="num">{entry.row.old}</span>
                <span class="num">{entry.row.new}</span>
                <span class="code mono"> {entry.row.text}</span>
              {:else if entry.row.kind === "delete"}
                <span class="num">{entry.row.old}</span>
                <span class="num"></span>
                <span class="code mono del"
                  >−{#each segments(entry.row.text, entry.row.inline) as part, i (i)}<span
                      class:word={part.changed}>{part.text}</span
                    >{/each}</span
                >
              {:else if entry.row.kind === "insert"}
                <span class="num"></span>
                <span class="num">{entry.row.new}</span>
                <span class="code mono add"
                  >+{#each segments(entry.row.text, entry.row.inline) as part, i (i)}<span
                      class:word={part.changed}>{part.text}</span
                    >{/each}</span
                >
              {/if}
            </div>
          {/each}
        {:else}
          {#each split.slice(range.start, range.end) as entry, index (range.start + index)}
            <div class="line" style:top="{(range.start + index) * ROW_HEIGHT}px">
              {#if entry.header}
                <span class="header mono">{entry.header}</span>
              {:else if entry.pair}
                <span class="num">{entry.pair.left?.line ?? ""}</span>
                <span class="code mono side" class:del={entry.pair.left?.kind === "delete"}
                  >{sign(entry.pair.left)}{#each cells(entry.pair.left) as part, i (i)}<span
                      class:word={part.changed}>{part.text}</span
                    >{/each}</span
                >
                <span class="num">{entry.pair.right?.line ?? ""}</span>
                <span class="code mono side" class:add={entry.pair.right?.kind === "insert"}
                  >{sign(entry.pair.right)}{#each cells(entry.pair.right) as part, i (i)}<span
                      class:word={part.changed}>{part.text}</span
                    >{/each}</span
                >
              {/if}
            </div>
          {/each}
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .diff {
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

  .eol {
    color: var(--text-secondary);
    font-size: 11px;
  }

  .warn {
    color: var(--status-modify);
    font-size: 11px;
  }

  .bar button {
    height: 20px;
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  .bar button:hover {
    background: var(--state-hover);
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

  .num {
    flex: 0 0 auto;
    width: 44px;
    padding-right: var(--sp-3);
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: 10px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .code {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .side {
    flex: 1 1 50%;
  }

  .word {
    border-radius: 2px;
    background: rgb(255 255 255 / 14%);
    font-weight: 600;
  }

  .code.del {
    background: var(--c-deleted-bg, rgb(90 40 40 / 35%));
    color: var(--status-delete);
  }

  .code.add {
    background: var(--c-added-bg, rgb(40 80 45 / 35%));
    color: var(--status-add);
  }

  .header {
    flex: 1 1 auto;
    padding-left: var(--sp-4);
    color: var(--status-ref);
    background: var(--surface-raised);
  }

  .message {
    margin: 0;
    padding: var(--sp-5);
    font-size: var(--fs-dense);
    color: var(--text-secondary);
  }
</style>
