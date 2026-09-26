<script lang="ts">
  import { summaryRows } from "$lib/diff-summary";
  import type { BlobSide } from "$lib/ipc";

  /** A file Diff does not show as lines: why, and each version by size and object id. */
  interface Props {
    reason: string;
    old: BlobSide | null;
    next: BlobSide | null;
  }

  let { reason, old, next }: Props = $props();

  const rows = $derived(summaryRows(old, next));
</script>

<div class="summary">
  <p class="reason">{reason}</p>
  <table>
    <tbody>
      {#each rows as row (row.label)}
        <tr>
          <th scope="row">{row.label}</th>
          <td class="size tabular">{row.size}</td>
          <td class="id">
            {#if row.id}<span class="mono">{row.id}</span>{:else if row.note}<span class="note">{row.note}</span>{/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .summary {
    padding: var(--sp-5);
    font-size: var(--fs-dense);
    color: var(--text-secondary);
  }

  .reason {
    margin: 0 0 var(--sp-4);
    color: var(--text-primary);
    user-select: text;
  }

  table {
    border-collapse: collapse;
  }

  th,
  td {
    padding: var(--sp-1) var(--sp-5) var(--sp-1) 0;
    text-align: left;
    font-weight: normal;
    vertical-align: baseline;
  }

  .size {
    color: var(--text-primary);
  }

  /* A hash is there to be copied into a terminal. */
  .id .mono {
    font-family: var(--font-mono);
    color: var(--text-primary);
    user-select: text;
  }

  .note {
    font-style: italic;
  }
</style>
