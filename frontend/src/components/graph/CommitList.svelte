<script lang="ts">
  import { formatCommitDate, shortOid } from "$lib/format";
  import { graph } from "$stores/graph.svelte";

  const LANE_WIDTH = 14;
  const GUTTER_PAD = 10;

  const gutterWidth = $derived(GUTTER_PAD + LANE_WIDTH * (graph.maxLane + 1));
</script>

{#if graph.error}
  <p class="error">{graph.error.message}</p>
{:else if graph.rows.length === 0}
  <p class="empty">{graph.loading ? "Reading history…" : "No commits yet."}</p>
{:else}
  <div class="list">
    {#each graph.rows as row (row.commit.oid)}
      <div class="row">
        <!-- Dots only for now; the Bézier lines move to a Canvas layer in T4.4. -->
        <span class="gutter" style:width="{gutterWidth}px">
          <span
            class="dot"
            class:merge={row.lane.kind === "merge"}
            class:root={row.lane.kind === "root"}
            style:left="{GUTTER_PAD + LANE_WIDTH * row.lane.lane}px"
            style:background="var(--c-lane-{(row.lane.color % 8) + 1})"
          ></span>
        </span>
        <span class="summary truncate">{row.commit.summary}</span>
        <span class="author truncate">{row.commit.authorName}</span>
        <span class="date tabular">{formatCommitDate(row.commit.timestamp, row.commit.tzOffsetMinutes)}</span>
        <span class="oid mono tabular">{shortOid(row.commit.oid)}</span>
      </div>
    {/each}
  </div>
  {#if graph.loading}
    <p class="empty">Loading more…</p>
  {/if}
{/if}

<style>
  .list {
    display: flex;
    flex-direction: column;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    height: var(--h-row-dense);
    padding-right: var(--sp-5);
    font-size: var(--fs-dense);
    white-space: nowrap;
  }

  .row:hover {
    background: var(--state-hover);
  }

  .gutter {
    position: relative;
    flex: 0 0 auto;
    align-self: stretch;
  }

  .dot {
    position: absolute;
    top: 50%;
    width: 7px;
    height: 7px;
    margin-top: -3.5px;
    margin-left: -3.5px;
    border-radius: 50%;
  }

  .dot.merge {
    width: 9px;
    height: 9px;
    margin-top: -4.5px;
    margin-left: -4.5px;
    box-shadow: 0 0 0 1.5px var(--surface-panel);
  }

  .dot.root {
    border-radius: 2px;
  }

  .summary {
    flex: 1 1 auto;
    min-width: 0;
  }

  .author {
    flex: 0 0 auto;
    max-width: 140px;
    color: var(--text-secondary);
  }

  .date {
    flex: 0 0 auto;
    color: var(--text-secondary);
    font-size: 11px;
  }

  .oid {
    flex: 0 0 auto;
    color: var(--text-secondary);
    font-size: 11px;
  }

  .empty,
  .error {
    margin: 0;
    padding: var(--sp-5);
    font-size: var(--fs-dense);
    color: var(--text-secondary);
  }

  .error {
    color: var(--status-delete);
    user-select: text;
  }
</style>
