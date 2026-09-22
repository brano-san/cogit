<script lang="ts">
  import MergeView from "$components/diff/MergeView.svelte";
  import TooltipLayer from "$components/common/TooltipLayer.svelte";
  import { parseMerge } from "$lib/merge-params";
  import { closeThisWindow, mergePreview, mergeResolved, resolveConflictText, type Region } from "$lib/ipc";
  import { suppressNativeMenu } from "$lib/native-menu";
  import { onWindowKey } from "$lib/child-window";
  import { settings } from "$stores/settings.svelte";

  const request = parseMerge(window.location.search);

  // Nothing in a Git client is a web page (R-127).
  $effect(() => suppressNativeMenu(document));

  let regions = $state.raw<Region[]>([]);
  let failed = $state<string | null>(null);

  $effect(() => {
    void settings.load();
    if (!request) return;
    void mergePreview(request.repo, request.path)
      .then((found) => (regions = found))
      .catch((err) => (failed = String(err)));
  });

  $effect(() => {
    document.title = request ? `${request.path} — Cogit` : "Merge — Cogit";
  });

  /** Written here, announced to the main window, and the window closes behind itself. */
  async function save(text: string) {
    if (!request) return;
    try {
      await resolveConflictText(request.repo, request.path, text);
      await mergeResolved(request.repo, request.path);
      await closeThisWindow();
    } catch (err) {
      failed = String(err);
    }
  }
</script>

<svelte:window onkeydown={onWindowKey} />

<TooltipLayer />

<div class="window">
  {#if !request}
    <p class="note">This window needs a conflicted file. Open it from the Files panel.</p>
  {:else if failed}
    <p class="note error">{failed}</p>
  {:else if regions.length === 0}
    <p class="note">Reading the three sides…</p>
  {:else}
    <MergeView
      path={request.path}
      {regions}
      saveShortcut
      onsave={(text) => void save(text)}
      oncancel={() => void closeThisWindow()}
    />
  {/if}
</div>

<style>
  .window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--surface-base);
    color: var(--text-primary);
  }

  .note {
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
