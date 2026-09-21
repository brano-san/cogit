<script lang="ts">
  import MergeView from "$components/diff/MergeView.svelte";
  import { parseMerge } from "$lib/merge-params";
  import { mergePreview, mergeResolved, resolveConflictText, type Region } from "$lib/ipc";
  import { settings } from "$stores/settings.svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  const request = parseMerge(window.location.search);

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
      await getCurrentWindow().close();
    } catch (err) {
      failed = String(err);
    }
  }
</script>

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
      onsave={(text) => void save(text)}
      oncancel={() => void getCurrentWindow().close()}
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
