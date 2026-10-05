<script lang="ts">
  import { DiffEditor } from "$lib/diff-editor";
  import { diffEdit } from "$stores/diff-edit.svelte";
  import { untrack } from "svelte";

  /** The Diff panel editing the working file (F-722): beside the diff's base, read-only, as
      the 2-way diff opens it; or alone, from the file menu's Edit. */
  interface Props {
    /** Back to the read-only diff (or the window closes); asks first about unsaved edits. */
    ondone: () => void;
    /** "Done" beside a diff; "Close" in a window of its own. */
    doneLabel?: string;
  }

  let { ondone, doneLabel = "Done" }: Props = $props();

  let host: HTMLDivElement | undefined = $state();
  let editor: DiffEditor | null = null;

  const opened = $derived(diffEdit.opened);
  const single = $derived(opened?.mode === "single");
  const baseCaption = $derived(
    opened?.spec.kind === "commitVsWorkTree" ? `${opened.spec.oid.slice(0, 7)}` : "Index",
  );

  // A new editor for each file and each reload; the text comes from the store once.
  $effect(() => {
    const target = host;
    const file = diffEdit.opened;
    void diffEdit.generation;
    if (!target || !file) return;
    const made = untrack(
      () =>
        new DiffEditor(target, file.mode === "single" ? null : file.base, file.text, {
          changed: () => (diffEdit.dirty = made.dirty),
          save: () => void save(),
          done: ondone,
        }),
    );
    editor = made;
    diffEdit.attach({ text: () => made.text(), focused: () => made.focused, save: () => void save() });
    void made.useLanguageOf(file.path);
    made.focusAt(file.at?.line ?? null, file.at?.column ?? 0);
    return () => {
      diffEdit.detach();
      made.destroy();
      if (editor === made) editor = null;
    };
  });

  async function save(force = false) {
    if (!editor) return;
    const text = editor.text();
    if (await diffEdit.save(text, force)) editor.markSaved();
  }
</script>

<div class="edit">
  <div class="bar">
    <span class="file truncate" title={opened?.path}>{opened?.path ?? ""}{diffEdit.dirty ? " *" : ""}</span>
    {#if !single}
      <span class="side base" title="The diff's left side: not a file you can write">
        <span aria-hidden="true">🔒</span>
        {baseCaption} · read-only
      </span>
      <span class="side">Working tree</span>
    {/if}
    <button type="button" class="btn sm" title="Undo (Ctrl+Z)" disabled={!opened} onclick={() => editor?.undo()}>Undo</button>
    <button type="button" class="btn sm" title="Redo (Ctrl+Y)" disabled={!opened} onclick={() => editor?.redo()}>Redo</button>
    <button
      type="button"
      class="btn sm"
      title="Write the file in its own encoding and line endings (Ctrl+S)"
      disabled={!diffEdit.dirty || diffEdit.saving}
      onclick={() => void save()}>Save</button
    >
    <button type="button" class="btn sm" title="{doneLabel} (Esc)" onclick={ondone}>{doneLabel}</button>
  </div>

  {#if diffEdit.changedOnDisk}
    <p class="banner" role="alert">
      The file changed on disk since you opened it.
      <button type="button" class="btn sm" onclick={() => void diffEdit.reload()} title="Take the file on disk; your edits go"
        >Reload</button
      >
      <button type="button" class="btn sm" onclick={() => void save(true)} title="Write your text over the file on disk"
        >Keep Mine</button
      >
    </p>
  {/if}
  {#if diffEdit.error}
    <p class="banner error" role="alert">{diffEdit.error}</p>
  {/if}

  {#if diffEdit.refused}
    <div class="refused">
      <p>{diffEdit.refused}</p>
      <button type="button" class="btn sm" onclick={ondone}>Back to the Diff</button>
    </div>
  {:else}
    <div class="host" bind:this={host}></div>
  {/if}
</div>

<style>
  .edit {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 0 auto;
    min-height: 32px;
    padding: var(--sp-2) var(--sp-4);
    background: var(--bg-elevated);
    border-bottom: 1px solid var(--border);
    font-size: var(--fs-dense);
  }

  .file {
    flex: 1 1 auto;
    min-width: 0;
    font-family: var(--font-mono);
  }

  .side {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--fg-secondary);
  }

  .banner {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    margin: 0;
    padding: var(--sp-2) var(--sp-4);
    background: var(--badge-warning-bg);
    color: var(--badge-warning-fg);
    font-size: var(--fs-dense);
  }

  .banner.error {
    background: var(--badge-error-bg);
    color: var(--badge-error-fg);
    user-select: text;
  }

  .host {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  .host :global(.cm-mergeView),
  .host :global(.cm-editor) {
    min-height: 100%;
  }

  .refused {
    padding: var(--sp-5);
    color: var(--fg-secondary);
  }
</style>
