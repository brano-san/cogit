<script lang="ts">
  import VirtualList from "$components/common/VirtualList.svelte";
  import { settings } from "$stores/settings.svelte";
  import { shortOid } from "$lib/format";
  import { BLAME_ROW_HEIGHT as ROW_HEIGHT, startsBlock } from "$lib/blame-window";
  import type { BlameLine } from "$lib/ipc";

  interface Props {
    lines: readonly BlameLine[];
    /** Index of the current line, whose history the window shows. */
    cursor: number;
    /** Commits whose lines `Highlight: Changes Since` marks. */
    highlighted: ReadonlySet<string>;
    onpick: (at: number) => void;
  }

  let { lines, cursor, highlighted, onpick }: Props = $props();
</script>

<VirtualList items={lines} rowHeight={ROW_HEIGHT} buffer={12} label="Blame" reveal={cursor}>
  {#snippet row(line, at)}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
      class="line"
      class:current={at === cursor}
      class:changed={highlighted.has(line.oid)}
      style:top="{at * ROW_HEIGHT}px"
      role="option"
      tabindex="-1"
      aria-selected={at === cursor}
      onclick={() => onpick(at)}
    >
      {#if startsBlock(lines, at)}
        <span class="annotation truncate" title="{line.summary} — {line.author}">
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

<style>
  .line {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    height: 18px;
    font-size: var(--fs-code);
    white-space: pre;
    cursor: default;
  }

  .line.changed {
    background: var(--c-modified-bg);
  }

  .line.current {
    background: var(--state-selected);
  }

  .annotation {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 0 auto;
    width: 240px;
    height: 100%;
    padding: 0 var(--sp-4);
    color: var(--text-secondary);
    font-size: 10px;
    border-right: 1px solid var(--divider);
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
    user-select: none;
  }

  .code {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
