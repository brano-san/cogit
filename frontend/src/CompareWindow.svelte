<script lang="ts">
  import { untrack } from "svelte";
  import DiffView from "$components/diff/DiffView.svelte";
  import ImageDiff from "$components/diff/ImageDiff.svelte";
  import SubmoduleDiff from "$components/diff/SubmoduleDiff.svelte";
  import TooltipLayer from "$components/common/TooltipLayer.svelte";
  import { installChildWindow } from "$lib/child-window";
  import { compareLabel, parseCompare } from "$lib/compare-params";
  import { loadCompare } from "$lib/compare-window";
  import { diff } from "$stores/diff.svelte";
  import { followSettings } from "$lib/settings-sync";
  import { settings } from "$stores/settings.svelte";

  const request = parseCompare(window.location.search);

  // No browser menu (R-127), and Esc / Ctrl+W close the window.
  $effect(() => installChildWindow(window));
  // What Preferences changes in the main window reaches this one too (F-335).
  $effect(() => followSettings(() => void settings.reload()));

  // Once, on opening: what the loads read must not make this effect run them again.
  $effect(() =>
    untrack(() => {
      if (request) void loadCompare(request, { settings, diff });
      else void settings.load();
    }),
  );

  const sides = $derived(request ? compareLabel(request.spec) : "");
  const title = $derived(request ? `${request.path} — ${sides}` : "Compare");

  $effect(() => {
    document.title = `${title} — Cogit`;
  });
</script>

<TooltipLayer />

<div class="window">
  {#if !request}
    <p class="note">
      This window needs a file to compare. Open it from the Diff panel rather than by hand.
    </p>
  {:else}
    <header>
      <span class="path truncate">{request.path}</span>
      <span class="spec">{sides}</span>
    </header>

    {#if diff.error}
      <p class="note error">{diff.error.message}</p>
    {:else if diff.diff?.kind === "submodule" && diff.shownPath}
      <SubmoduleDiff
        path={diff.shownPath}
        recorded={diff.diff.recorded}
        previous={diff.diff.previous}
        checkedOut={diff.diff.checkedOut}
        inIndex={diff.diff.inIndex}
      />
    {:else if diff.diff?.kind === "image"}
      <ImageDiff
        before={diff.images[0]}
        after={diff.images[1]}
        oldSize={diff.diff.oldSize}
        newSize={diff.diff.newSize}
        mime={diff.diff.mime}
      />
    {:else if diff.diff && diff.shownPath}
      <DiffView
        diff={diff.diff}
        path={diff.shownPath}
        stageable={false}
        onstage={() => {}}
        whitespace={diff.whitespace}
        onwhitespace={(mode) => void diff.setWhitespace(request.repo, mode)}
      />
    {:else}
      <p class="note">Loading…</p>
    {/if}
  {/if}
</div>

<style>
  .window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--surface-base);
    color: var(--text-primary);
    font-family: var(--font-ui);
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--sp-5);
    height: var(--h-panel-hdr);
    padding: 0 var(--sp-5);
    background: var(--surface-panel);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-dense);
  }

  .path {
    flex: 1 1 auto;
    min-width: 0;
    font-family: var(--font-mono);
  }

  .spec {
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  .note {
    margin: 0;
    padding: var(--sp-6);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .error {
    color: var(--status-delete);
  }
</style>
