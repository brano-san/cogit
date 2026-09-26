<script lang="ts">
  import Avatar from "$components/common/Avatar.svelte";
  import FoldToggle from "$components/graph/FoldToggle.svelte";
  import RefCapsule from "$components/graph/RefCapsule.svelte";
  import { capsules, dateTooltip, refLabelKey, shortOid, type RefLabel } from "$lib/format";
  import { linkStubs, linkTitle } from "$lib/graph-links";
  import { COLUMN_WIDTH, graphTime, timeWidth, type GraphColumn, type GraphTimeFormat } from "$lib/graph-row";
  import type { GraphEntry } from "$lib/graph-wire";
  import { overlapLabel, overlapTooltip } from "$lib/overlap";
  import { commit as selection } from "$stores/commit.svelte";
  import { graphFolds } from "$stores/graph-folds.svelte";
  import { graphOverlays } from "$stores/graph-overlay.svelte";
  import { overlap } from "$stores/overlap.svelte";

  /** What a commit's row in the graph shows, inside the row the list positions. */
  interface Props {
    entry: GraphEntry;
    labels: readonly RefLabel[];
    /** Merged branches fold and the list is the graph, not a filtered one. */
    folds: boolean;
    cells: readonly (GraphColumn | "overlap")[];
    timeFormat: GraphTimeFormat;
    /** What relative times count from. */
    now: number;
    clipX: number;
    onrefmenu?: (label: RefLabel, event: MouseEvent) => void;
    describeEnd: (oid: string) => { short: string; summary: string | null };
    onprefetch: (oids: readonly string[]) => void;
    onjump: (oid: string | undefined) => void;
  }

  let { entry, labels, folds, cells, timeFormat, now, clipX, onrefmenu, describeEnd, onprefetch, onjump }: Props =
    $props();

  /** Enough for HEAD plus its upstream plus a tag; the rest fold into a `+N` capsule. */
  const CAPSULE_ROOM = 3;

  const refs = $derived(capsules(labels, CAPSULE_ROOM));
  const timeColumn = $derived(timeWidth(timeFormat));
</script>

{#if folds}
  {@const hidden = graphOverlays.foldAt(entry.layout.row)}
  {@const open = graphFolds.expanded.has(entry.commit.oid)}
  {#if hidden > 0 || open}
    <FoldToggle {open} {hidden} ontoggle={() => graphFolds.toggle(entry.commit.oid)} />
  {/if}
{/if}
{#each refs.shown as label (refLabelKey(label))}
  <RefCapsule {label} onmenu={onrefmenu && ((event) => onrefmenu(label, event))} />
{/each}
{#if refs.hidden.length > 0}
  <span class="capsule more" title={refs.hidden.map((l) => l.text).join("\n")}>+{refs.hidden.length}</span>
{/if}
<span class="summary truncate">{entry.commit.summary}</span>
{#each cells as cell (cell)}
  {#if cell === "author"}
    <span class="author truncate" style:max-width="{COLUMN_WIDTH.author}px">{entry.commit.authorName}</span>
  {:else if cell === "avatar"}
    <Avatar name={entry.commit.authorName} email={entry.commit.authorEmail} />
  {:else if cell === "time"}
    <span
      class="date time tabular truncate"
      style:flex-basis="{timeColumn}px"
      title={dateTooltip(entry.commit.timestamp, entry.commit.tzOffsetMinutes)}
      >{graphTime(entry.commit.timestamp, entry.commit.tzOffsetMinutes, now, timeFormat)}</span
    >
  {:else if cell === "overlap"}
    {@const row = overlap.rowOf(entry.commit.oid, selection.oid)}
    <span
      class="overlap {row?.overlap ?? 'none'}"
      class:base={row?.isBase}
      style:flex-basis="{COLUMN_WIDTH.overlap}px"
      title={row ? overlapTooltip(row.shared, row.sharedTotal) : ""}
    >
      {row?.isBase ? "base" : row ? overlapLabel(row.overlap) : ""}
    </span>
  {:else}
    <span class="oid mono tabular" style:flex-basis="{COLUMN_WIDTH.hash}px">{shortOid(entry.commit.oid)}</span>
  {/if}
{/each}
{#each linkStubs(entry.layout) as stub (stub.segment)}
  {#if stub.box.left + stub.box.size <= clipX}
    <button
      type="button"
      class="link-stub"
      tabindex="-1"
      style:left="{stub.box.left}px"
      style:top="{stub.box.top}px"
      style:width="{stub.box.size}px"
      style:height="{stub.box.size}px"
      aria-label="Go to the other end of this link"
      title={linkTitle(stub.oids, describeEnd)}
      onpointerenter={() => onprefetch(stub.oids)}
      onclick={(event) => {
        event.stopPropagation();
        onjump(stub.oids[0]);
      }}
    ></button>
  {/if}
{/each}

<style>
  .capsule.more {
    background: var(--surface-raised);
    color: var(--text-secondary);
  }

  .capsule {
    flex: 0 0 auto;
    height: 16px;
    padding: 0 var(--sp-3);
    border: 1px solid;
    border-radius: var(--r-md);
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 14px;
  }

  /* Gives way after the branch labels, which shrink first (#12, R-331); the right columns
     never do, and past its room the graph area is cut instead. */
  .summary {
    flex: 1 1 auto;
    min-width: 0;
  }

  /* Widths of the columns are inline, from `COLUMN_WIDTH`: the row measures them too. */
  .author {
    flex: 0 0 auto;
    color: var(--text-secondary);
  }

  .date,
  .overlap {
    flex: 0 0 auto;
    color: var(--text-secondary);
    font-size: var(--fs-header);
    text-align: right;
  }

  /* Beside the avatar, one row gap from it (#14): right-aligned in its fixed column, a short
     date sat a whole column away from the face it belongs to. */
  .date.time {
    text-align: left;
  }

  .overlap.heavy {
    color: var(--status-modify);
  }

  .overlap.same {
    color: var(--status-delete);
  }

  .overlap.base {
    color: var(--status-ref);
    font-weight: 600;
  }

  .oid {
    flex: 0 0 auto;
    overflow: hidden;
    color: var(--text-secondary);
    font-size: 11px;
  }

  /* Over the arrow of a cut link, under the canvas that draws it (R-330). */
  .link-stub {
    position: absolute;
    padding: 0;
    background: none;
    border: 0;
    cursor: pointer;
  }
</style>
