<script lang="ts">
  import { untrack } from "svelte";
  import DiffView from "$components/diff/DiffView.svelte";
  import ImageDiff from "$components/diff/ImageDiff.svelte";
  import TooltipLayer from "$components/common/TooltipLayer.svelte";
  import Notifications from "$components/layout/Notifications.svelte";
  import { errors } from "$stores/errors.svelte";
  import { runMutation, type MutationContext } from "$lib/mutation";
  import { announceTreeChange } from "$lib/tree-sync";
  import { installChildWindow } from "$lib/child-window";
  import { compareLabel, diffWindowTitle, parseCompare, sideCaptions } from "$lib/compare-params";
  import { firstParent, handOverModule, loadCompare } from "$lib/compare-window";
  import { closeThisWindow } from "$lib/ipc";
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

  /** Unknown until read; the header names the commit meanwhile. */
  let parent = $state<string | null | undefined>(undefined);
  $effect(() =>
    untrack(() => {
      if (request) void firstParent(request).then((found) => (parent = found));
    }),
  );

  const sides = $derived(request ? compareLabel(request.spec, parent) : null);
  const captions = $derived(request ? sideCaptions(request.spec, parent) : undefined);

  /** The window's own title is the application and the file's name; the header has the path. */
  const title = $derived(request ? diffWindowTitle(diff.shownPath ?? request.path) : "Cogit — Compare");

  $effect(() => {
    document.title = title;
    // The native title bar is not the page's `<title>`; outside the app (a browser) there is none.
    void import("@tauri-apps/api/window").then(({ getCurrentWindow }) => getCurrentWindow().setTitle(title)).catch(() => {});
  });

  /** Stage, Unstage and Discard here end like the Diff panel's: through `runMutation`, a
      failure goes to the notification queue with git's own words, and on success the diff
      is read again and the main window reads its lists (its watcher is quiet after our own
      writes). */
  const mutation: MutationContext = {
    repo: () => request?.repo ?? null,
    epoch: () => 0,
    report: (err) => errors.report(err, "Could not change the working tree"),
    loadWorktree: async () => {},
    after: async (paths) => {
      await diff.reload();
      if (request) await announceTreeChange({ repo: request.repo, path: paths[0] ?? request.path });
    },
  };
  diff.useMutation(mutation);

  function stageLines(selected: ReadonlySet<string>, reverse: boolean) {
    const path = diff.shownPath;
    if (path) void runMutation(mutation, () => diff.stageLines(selected, reverse), [path], false);
  }

  // The file is gone from both sides (Discard removed it): nothing left to compare, and the
  // window stops asking for it. The note shows for the moment the close takes.
  $effect(() => {
    if (diff.gone) void closeThisWindow().catch(() => {});
  });

  // A submodule has nothing to compare line by line: the main window opens it (R-537).
  let handedOver = false;
  $effect(() => {
    if (diff.diff?.kind !== "submodule" || !request || handedOver) return;
    handedOver = true;
    untrack(() => void handOverModule(request));
  });
</script>

<TooltipLayer />
<Notifications onopenurl={() => {}} onshowoutput={() => {}} onaction={() => {}} />

<div class="window">
  {#if !request}
    <p class="note">
      This window needs a file to compare. Open it from the Diff panel rather than by hand.
    </p>
  {:else}
    <header>
      <span class="path truncate" title={sides?.tip}>{request.path}</span>
    </header>

    {#if diff.gone}
      <p class="note">File deleted</p>
    {:else if diff.error}
      <p class="note error">{diff.error.message}</p>
    {:else if diff.diff?.kind === "submodule"}
      <p class="note">{request.path} is a submodule: it opens in the main window.</p>
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
        stageable={diff.stageable}
        showPath={false}
        {captions}
        onstage={stageLines}
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
