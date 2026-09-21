<script lang="ts">
  import VirtualList from "$components/common/VirtualList.svelte";
  import { settings } from "$stores/settings.svelte";
  import { shortOid } from "$lib/format";
  import type { BlameLine } from "$lib/ipc";

  interface Props {
    lines: readonly BlameLine[];
    path: string;
    onselect: (oid: string) => void;
  }

  let { lines, path, onselect }: Props = $props();

  const ROW_HEIGHT = 18;

  /** Only the first line of a run shows the annotation, as `git blame` does. */
  function startsRun(at: number): boolean {
    return at === 0 || lines[at - 1]?.oid !== lines[at]?.oid;
  }
</script>

<div class="blame">
  <div class="bar">
    <span class="path mono truncate">{path}</span>
    <span class="count tabular">{lines.length} lines</span>
  </div>

  <VirtualList items={lines} rowHeight={ROW_HEIGHT} buffer={12} label="Blame">
    {#snippet row(line, at)}
        <div class="line" style:top="{at * ROW_HEIGHT}px">
          {#if startsRun(at)}
            <span
              class="annotation truncate"
              role="button"
              tabindex="-1"
              title="{line.summary} — {line.author}"
              onclick={() => onselect(line.oid)}
              onkeydown={(e) => e.key === "Enter" && onselect(line.oid)}
            >
              <span class="oid mono">{shortOid(line.oid)}</span>
              <span class="author truncate">{line.author}</span>
              <span class="date tabular">{settings.formatDate(line.timestamp, 0)}</span>
            </span>
          {:else}
            <span class="annotation"></span>
          {/if}
          <span class="num tabular">{line.line}</span>
          <span class="code mono">{line.text}</span>
        </div>
    {/snippet}
  </VirtualList>
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
