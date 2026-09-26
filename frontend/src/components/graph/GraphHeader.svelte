<script lang="ts">
  import GraphFilter from "$components/graph/GraphFilter.svelte";
  import GraphNav from "$components/graph/GraphNav.svelte";
  import GraphOptions from "$components/graph/GraphOptions.svelte";
  import type { CommitQuery } from "$lib/ipc";

  /** The Graph panel's header controls: the filter, Home and Back, the options (doc/05 §3.4). */
  interface Props {
    onchange: (query: CommitQuery) => void;
    matches?: number;
    query: CommitQuery;
    /** Opens Preferences on the graph's page. */
    onpreferences: () => void;
  }

  let { onchange, matches, query, onpreferences }: Props = $props();
</script>

<div class="header">
  <GraphFilter {onchange} {matches} {query} />
  <GraphNav />
  <GraphOptions {onpreferences} />
</div>

<style>
  .header {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }
</style>
