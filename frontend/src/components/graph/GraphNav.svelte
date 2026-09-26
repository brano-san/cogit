<script lang="ts">
  import { homeTarget, type Selected } from "$lib/selection-history";
  import { commit as selection } from "$stores/commit.svelte";
  import { graph } from "$stores/graph.svelte";
  import { graphNav } from "$stores/graph-nav.svelte";
  import { repository } from "$stores/repository.svelte";

  /** Home and Back beside the graph filter (F-562). */
  const headOid = $derived.by(() => {
    const head = repository.current?.head;
    return head && head.kind !== "unborn" ? head.oid : null;
  });

  $effect(() => graphNav.track(repository.current?.repo ?? null, selection.oid));

  const homeOff = $derived(selection.oid === null && headOid === null ? "No commit yet" : null);
  const homeTitle = $derived(
    homeOff ?? (selection.oid === null ? "Go to the HEAD commit" : "Go to the Working Tree"),
  );

  function show(target: Selected) {
    const repo = repository.current?.repo;
    if (repo === undefined || graph.stale) return;
    if (target === null) {
      selection.showWorkingTree();
      graphNav.toTop();
      return;
    }
    void selection.select(repo, target);
    graph.requestReveal(target);
  }

  function home() {
    if (homeOff === null) show(homeTarget(selection.oid, headOid));
  }

  function back() {
    const target = graphNav.back();
    if (target !== undefined) show(target);
  }
</script>

<div class="nav">
  <button
    type="button"
    class="tool"
    class:dead={homeOff !== null}
    aria-disabled={homeOff !== null}
    title={homeTitle}
    aria-label={homeTitle}
    onclick={home}
  >
    <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2.5 7.5 8 2.5l5.5 5M4 6.5v7h3v-4h2v4h3v-7" /></svg>
  </button>
  <button
    type="button"
    class="tool"
    class:dead={!graphNav.canGoBack}
    aria-disabled={!graphNav.canGoBack}
    title={graphNav.canGoBack ? "Back to the selection before" : "Nothing selected before"}
    aria-label="Back"
    onclick={back}
  >
    <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M13.5 8h-11M6.5 4 2.5 8l4 4" /></svg>
  </button>
</div>

<style>
  .nav {
    display: flex;
    flex: 0 0 auto;
    gap: var(--sp-1);
  }

  .tool {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--h-button-sm);
    height: var(--h-button-sm);
    padding: 0;
    background: none;
    color: var(--text-secondary);
    border: 0;
    border-radius: var(--r-sm);
    cursor: default;
  }

  .tool:hover:not(.dead) {
    background: var(--state-hover);
    color: var(--text-primary);
  }

  .tool.dead {
    opacity: 0.4;
  }

  svg {
    width: var(--panel-icon);
    height: var(--panel-icon);
    fill: none;
    stroke: currentColor;
    stroke-width: 1.3;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
