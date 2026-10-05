<script lang="ts">
  import { DiffEditor } from "$lib/diff-editor";
  import { diffEdit } from "$stores/diff-edit.svelte";
  import { untrack } from "svelte";

  /** The Diff panel in Edit mode: the diff's base read-only on the left, the working file
      editable on the right, re-diffed while typing (F-722). */
  interface Props {
    /** Back to the read-only diff; asks first when there are unsaved edits. */
    ondone: () => void;
  }

  let { ondone }: Props = $props();

  let host: HTMLDivElement | undefined = $state();
  let editor: DiffEditor | null = null;

  const opened = $derived(diffEdit.opened);
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
        new DiffEditor(target, file.base, file.text, {
          changed: () => (diffEdit.dirty = made.dirty),
          save: () => void save(),
        }),
    );
    editor = made;
    diffEdit.attach(() => made.text(), { focused: () => made.focused, save: () => void save() });
    void made.useLanguageOf(file.path);
    made.focus();
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
    <span class="side base" title="The diff's left side: not a file you can write">
      <span aria-hidden="true">🔒</span>
      {baseCaption} · read-only
    </span>
    <span class="side work">
      Working tree{#if diffEdit.dirty}<span class="dirty" title="Unsaved edits"> ●</span>{/if}
    </span>
    <button type="button" class="btn sm" title="Undo (Ctrl+Z)" disabled={!opened} onclick={() => editor?.undo()}>Undo</button>
    <button type="button" class="btn sm" title="Redo (Ctrl+Y)" disabled={!opened} onclick={() => editor?.redo()}>Redo</button>
    <button
      type="button"
      class="btn sm"
      title="Write the file in its own encoding and line endings (Ctrl+S)"
      disabled={!diffEdit.dirty || diffEdit.saving}
      onclick={() => void save()}>Save</button
    >
    <button type="button" class="btn sm" title="Back to the diff" onclick={ondone}>Done</button>
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

  .side {
    flex: 1 1 0;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .base {
    color: var(--fg-secondary);
  }

  .dirty {
    color: var(--status-modify);
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

  .host :global(.cm-mergeView) {
    min-height: 100%;
  }

  .refused {
    padding: var(--sp-5);
    color: var(--fg-secondary);
  }
</style>
