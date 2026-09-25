<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import ConfirmDialog from "$components/common/ConfirmDialog.svelte";
  import MergeView from "$components/diff/MergeView.svelte";
  import TooltipLayer from "$components/common/TooltipLayer.svelte";
  import { failureText, parseMerge } from "$lib/merge-params";
  import { closeThisWindow, mergePreview, mergeResolved, resolveConflictText, type Region } from "$lib/ipc";
  import { closeGuard, installChildWindow } from "$lib/child-window";
  import { confirmation } from "$stores/confirm.svelte";
  import { settings } from "$stores/settings.svelte";

  const request = parseMerge(window.location.search);

  // No browser menu (R-127), and Esc / Ctrl+W close the window.
  $effect(() => installChildWindow(window));

  let view = $state<ReturnType<typeof MergeView>>();
  /** Written to the index: the window closes behind its own Save without asking. */
  let saved = false;

  // Esc, Ctrl+W, Cancel and the ✕ all come here as one close request (04 §7).
  $effect(() => {
    const guard = closeGuard(
      () => !saved && (view?.unsaved() ?? false),
      () =>
        confirmation.ask({
          title: "Discard the Resolution",
          message: "The sides picked and the edits to the result are not saved. Close the window and lose them?",
          confirm: "Discard",
          warning: true,
        }),
    );
    const pending = getCurrentWindow().onCloseRequested(guard);
    return () => void pending.then((stop) => stop()).catch(() => {});
  });

  let regions = $state.raw<Region[]>([]);
  /** Nothing to show without the three sides, so this one takes the window. */
  let loadFailed = $state<string | null>(null);
  /** Shown above the editor, which keeps the choices and the edits for another Save. */
  let saveFailed = $state<string | null>(null);

  $effect(() => {
    void settings.load();
    if (!request) return;
    void mergePreview(request.repo, request.path)
      .then((found) => (regions = found))
      .catch((err) => (loadFailed = failureText(err)));
  });

  $effect(() => {
    document.title = request ? `${request.path} — Cogit` : "Merge — Cogit";
  });

  /** Written here, announced to the main window, and the window closes behind itself. */
  async function save(text: string) {
    if (!request) return;
    saveFailed = null;
    try {
      await resolveConflictText(request.repo, request.path, text);
      await mergeResolved(request.repo, request.path);
      saved = true;
      await closeThisWindow();
    } catch (err) {
      saveFailed = failureText(err);
    }
  }
</script>

<TooltipLayer />

<div class="window">
  {#if !request}
    <p class="note">This window needs a conflicted file. Open it from the Files panel.</p>
  {:else if loadFailed}
    <p class="note error">{loadFailed}</p>
  {:else if regions.length === 0}
    <p class="note">Reading the three sides…</p>
  {:else}
    {#if saveFailed}
      <div class="failure" role="alert">
        <p>The resolution was not saved. It is still here: fix the cause and save again.</p>
        <pre class="error">{saveFailed}</pre>
      </div>
    {/if}
    <MergeView
      bind:this={view}
      path={request.path}
      {regions}
      saveShortcut
      onsave={(text) => void save(text)}
      oncancel={() => void closeThisWindow()}
    />
  {/if}
</div>

{#if confirmation.open}
  <ConfirmDialog
    title={confirmation.open.title}
    message={confirmation.open.message}
    confirm={confirmation.open.confirm}
    warning={confirmation.open.warning}
    onanswer={(yes) => confirmation.answer(yes)}
  />
{/if}

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

  .failure {
    flex: none;
    padding: var(--sp-3) var(--sp-5);
    font-size: var(--fs-dense);
    background: var(--surface-raised);
    border-bottom: 1px solid var(--divider);
  }

  .failure p {
    margin: 0 0 var(--sp-2);
  }

  .failure pre {
    max-height: 10em;
    margin: 0;
    overflow: auto;
    font-family: var(--font-mono);
    white-space: pre-wrap;
  }
</style>
