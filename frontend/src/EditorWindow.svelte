<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { untrack } from "svelte";
  import DiffEditPane from "$components/diff/DiffEditPane.svelte";
  import UnsavedDialog from "$components/common/UnsavedDialog.svelte";
  import TooltipLayer from "$components/common/TooltipLayer.svelte";
  import { closeGuard, installChildWindow } from "$lib/child-window";
  import { closeThisWindow } from "$lib/ipc";
  import { followSettings } from "$lib/settings-sync";
  import { diffEdit } from "$stores/diff-edit.svelte";
  import { settings } from "$stores/settings.svelte";
  import { unsavedPrompt } from "$stores/unsaved-prompt.svelte";
  import { webMenus } from "$stores/web-menus.svelte";

  /** Edit (F-722): one working-tree file in the built-in editor, alone, in its own window.
      Saving writes the file in its own shape; the main window reads its lists again from
      the watcher, which sees the write. */
  const params = new URLSearchParams(window.location.search);
  const repo = Number(params.get("repo"));
  const path = params.get("path") ?? "";
  const valid = Number.isInteger(repo) && repo > 0 && path !== "";
  const win = getCurrentWindow();

  $effect(() => installChildWindow(window));
  $effect(() => followSettings(() => void settings.reload()));
  $effect(() =>
    untrack(() => {
      void settings.load();
      if (valid) void diffEdit.start(repo as never, { kind: "workTreeVsIndex" }, path, "single");
    }),
  );

  const name = path.slice(path.lastIndexOf("/") + 1);
  const title = $derived(`${name}${diffEdit.dirty ? " *" : ""} — Edit — Cogit`);
  $effect(() => {
    document.title = title;
    void win.setTitle(title).then(() => webMenus.refreshTitle()).catch(() => {});
  });

  // The ✕, Esc, Ctrl+W and Close all come here: unsaved edits are saved or dropped on purpose.
  $effect(() => {
    const guard = closeGuard(
      () => diffEdit.dirty,
      async () => {
        const choice = await unsavedPrompt.ask("Unsaved Edits", path);
        if (choice === "cancel") return false;
        if (choice === "save") return diffEdit.save(diffEdit.currentText());
        return true;
      },
    );
    const pending = win.onCloseRequested(guard);
    return () => void pending.then((stop) => stop()).catch(() => {});
  });
</script>

<TooltipLayer />

<div class="window">
  {#if valid}
    <DiffEditPane doneLabel="Close" ondone={() => void closeThisWindow()} />
  {:else}
    <p class="note">This window needs a file to edit. Open it with Edit on a file in the Files panel.</p>
  {/if}
</div>

{#if unsavedPrompt.open}
  <UnsavedDialog
    title={unsavedPrompt.open.title}
    path={unsavedPrompt.open.path}
    cancellable={unsavedPrompt.open.cancellable}
    onanswer={(choice) => unsavedPrompt.answer(choice)}
  />
{/if}

<style>
  .window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg-editor);
  }

  .note {
    padding: var(--sp-5);
    color: var(--fg-secondary);
  }
</style>
