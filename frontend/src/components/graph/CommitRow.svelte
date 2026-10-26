<script lang="ts">
  import HereMark from "$components/graph/HereMark.svelte";
  import { repository } from "$stores/repository.svelte";
  import Avatar from "$components/common/Avatar.svelte";
  import FoldToggle from "$components/graph/FoldToggle.svelte";
  import RefCapsule from "$components/graph/RefCapsule.svelte";
  import { capsules, dateTooltip, refLabelKey, shortOid, type RefLabel } from "$lib/format";
  import { linkStubs, linkTitle } from "$lib/graph-links";
  import { COLUMN_WIDTH, graphTime, type GraphColumn, type GraphTimeFormat } from "$lib/graph-row";
  import { timeColumn as timeColumnStore } from "$stores/time-column.svelte";
  import type { GraphEntry } from "$lib/graph-wire";
  import { overlapLabel, overlapTooltip } from "$lib/overlap";
  import { commit as selection } from "$stores/commit.svelte";
  import { graphFolds } from "$stores/graph-folds.svelte";
  import { graphOverlays } from "$stores/graph-overlay.svelte";
  import { overlap } from "$stores/overlap.svelte";
  import { avatars } from "$stores/avatars.svelte";
  import { notedCommitsStore } from "$stores/noted-commits.svelte";
  import { settings } from "$stores/settings.svelte";
  import { signatures } from "$stores/signatures.svelte";
  import { signatureBadge, signatureLabel } from "$lib/signature";

  /** What a commit's row in the graph shows, inside the row the list positions. */
  interface Props {
    entry: GraphEntry;
    labels: readonly RefLabel[];
    /** HEAD points here, on a branch or detached: the subject is bold. */
    head?: boolean;
    /** Merged branches fold and the list is the graph, not a filtered one. */
    folds: boolean;
    cells: readonly (GraphColumn | "overlap")[];
    timeFormat: GraphTimeFormat;
    author?: boolean;
    avatar?: boolean;
    /** What relative times count from. */
    now: number;
    clipX: number;
    onrefmenu?: (label: RefLabel, event: MouseEvent) => void;
    onrefactivate?: (label: RefLabel) => void;
    describeEnd: (oid: string) => { short: string; summary: string | null };
    onprefetch: (oids: readonly string[]) => void;
    onjump: (oid: string | undefined) => void;
  }

  let {
    entry,
    labels,
    head = false,
    folds,
    cells,
    timeFormat,
    author = true,
    avatar = true,
    now,
    clipX,
    onrefmenu,
    onrefactivate,
    describeEnd,
    onprefetch,
    onjump,
  }: Props = $props();

  /** Enough for HEAD plus its upstream plus a tag; the rest fold into a `+N` capsule. */
  const CAPSULE_ROOM = 3;

  const refs = $derived(capsules(labels, CAPSULE_ROOM));
  /** A detached HEAD on this commit: the mark goes before its subject, a branch's goes in
      its label. */
  const detachedHere = $derived(
    repository.current?.head.kind === "detached" && repository.current.head.oid === entry.commit.oid,
  );
  const timeColumn = $derived(timeColumnStore.width(timeFormat));
  const hashColumn = $derived(timeColumnStore.hashWidth());
  const signature = $derived(signatures.statuses.get(entry.commit.oid));
  const badge = $derived(signatureBadge(signature?.status));
</script>

{#if folds}
  {@const hidden = graphOverlays.foldAt(entry.layout.row)}
  {@const open = graphFolds.expanded.has(entry.commit.oid)}
  {#if hidden > 0 || open}
    <FoldToggle {open} {hidden} ontoggle={() => graphFolds.toggle(entry.commit.oid)} />
  {/if}
{/if}
<!-- Labels in full, cut by this area's edge rather than pushing the right columns (R-331). -->
<span class="middle">
{#each refs.shown as label (refLabelKey(label))}
  <RefCapsule
    {label}
    onmenu={onrefmenu && ((event, ref) => onrefmenu(ref, event))}
    onactivate={onrefactivate && ((ref) => onrefactivate(ref))}
  />
{/each}
{#if refs.hidden.length > 0}
  <span class="capsule more" title={refs.hidden.map((l) => l.text).join("\n")}>+{refs.hidden.length}</span>
{/if}
{#if settings.current.graphShowNotes && notedCommitsStore.has(entry.commit.oid)}
  <span
    class="note-icon"
    role="img"
    aria-label="Git note"
    data-tip={notedCommitsStore.texts.get(entry.commit.oid) ?? ""}
    onpointerenter={() => void notedCommitsStore.load(entry.commit.oid)}
  >
    <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M9 2H3.5v12h6" />
      <path d="M3.5 2h5.5" />
      <path d="M5.5 5.5h2.5M5.5 8h1.5" />
      <path d="M13.6 5.4l-5 5-.6 2.2 2.2-.6 5-5z" />
    </svg>
  </span>
{/if}
{#if badge && signature}
  <span class="signature {badge}" role="img" aria-label={signatureLabel(signature)} data-tip={signatureLabel(signature)}>
    <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M8 1.8l5 1.9v4c0 3.1-2.1 5.4-5 6.5-2.9-1.1-5-3.4-5-6.5v-4z" />
      {#if badge === "verified"}
        <path d="M5.6 8l1.7 1.7 3.2-3.4" />
      {:else if badge === "bad"}
        <path d="M6 6l4 4M10 6l-4 4" />
      {:else}
        <path d="M8 5.2v3.2M8 10.6v.1" />
      {/if}
    </svg>
  </span>
{/if}
{#if detachedHere}<HereMark />{/if}<span class="summary truncate" class:head>{entry.commit.summary}</span>
</span>
{#each cells as cell, at (cell)}
  {#if cell === "author"}
    <span class="author truncate" style:max-width="{COLUMN_WIDTH.author}px">{author ? entry.commit.authorName : ""}</span>
  {:else if cell === "avatar"}
    {#if avatar}
      <Avatar name={entry.commit.authorName} email={entry.commit.authorEmail} />
    {:else if avatars.enabled}
      <span class="no-avatar" style:width="{COLUMN_WIDTH.avatar}px"></span>
    {/if}
  {:else if cell === "time"}
    <span
      class="date time tabular truncate"
      class:last={at === cells.length - 1}
      style:flex-basis="calc({timeColumn}px + var(--sp-2))"
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
    <span class="oid mono tabular" style:flex-basis="{hashColumn}px">{shortOid(entry.commit.oid)}</span>
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
  .middle {
    display: flex;
    flex: 1 1 auto;
    align-items: center;
    gap: var(--sp-3);
    min-width: 0;
    overflow: hidden;
  }

  .no-avatar {
    flex: none;
  }

  .note-icon {
    display: inline-flex;
    flex: none;
    align-items: center;
    color: var(--text-secondary);
  }

  .note-icon:hover {
    color: var(--text-primary);
  }

  .signature {
    display: inline-flex;
    flex: none;
    align-items: center;
    color: var(--text-secondary);
  }

  .signature.verified {
    color: var(--status-add);
  }

  .signature.bad {
    color: var(--status-delete);
  }

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
    font-size: var(--fs-small);
    line-height: 14px;
  }

  /* Gives way first; the branch labels never shrink, `.middle` cuts them at its edge, and
     the right columns never move. */
  .summary {
    flex: 1 1 auto;
    min-width: 0;
  }

  .summary.head {
    font-weight: 700;
  }

  /* Widths of the columns are inline, from `COLUMN_WIDTH`: the row measures them too. */
  /* Only the subject gives way: the author keeps its name up to its column's cap (cut past
     it), and the date is measured to fit. */
  .author {
    flex: 0 0 auto;
    min-width: 0;
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

  /* With nothing after it the date is the row's edge, and ends where the hash would: a
     left-aligned short date there left its empty column as a gap before the edge. */
  .date.time.last {
    text-align: right;
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
    font-size: var(--fs-header);
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
