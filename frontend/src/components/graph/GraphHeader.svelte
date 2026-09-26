<script lang="ts">
  import GraphFilter from "$components/graph/GraphFilter.svelte";
  import GraphNav from "$components/graph/GraphNav.svelte";
  import type { CommitQuery } from "$lib/ipc";

  /** The Graph panel's header controls: the filter, then Home and Back (doc/05 §3.4). */
  interface Props {
    onchange: (query: CommitQuery) => void;
    matches?: number;
    query: CommitQuery;
  }

  let { onchange, matches, query }: Props = $props();
</script>

<div class="header">
  <GraphFilter {onchange} {matches} {query} />
  <GraphNav />
</div>

<style>
  .header {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }
</style>
