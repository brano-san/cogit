<script lang="ts">
  import { DiffEditor } from "$lib/diff-editor";
  import { diffEdit } from "$stores/diff-edit.svelte";
  import { untrack } from "svelte";
  import EditBanners from "./EditBanners.svelte";

  /** The working file alone, in the Edit window (F-722). In a diff the file is its right
      side instead (`DiffView`); both edit through `diff-editor`. */
  interface Props {
    /** The window closes, asking first about unsaved edits. */
    ondone: () => void;
    doneLabel?: string;
  }

  let { ondone, doneLabel = "Close" }: Props = $props();

  let host: HTMLDivElement | undefined = $state();
  let editor: DiffEditor | null = null;

  const opened = $derived(diffEdit.opened);
  /** A new editor for the file and for each reload; a save keeps the one there is. */
  const loaded = $derived(opened === null ? null : diffEdit.generation);

  $effect(() => {
    const target = host;
    if (!target || loaded === null) return;
    const made = untrack(() => {
      const file = diffEdit.opened!;
      const created: DiffEditor = new DiffEditor(target, file.text, {
        changed: () => (diffEdit.dirty = created.dirty),
        save: () => void save(),
        done: ondone,
      });
      void created.useLanguageOf(file.path);
      return created;
    });
    editor = made;
    diffEdit.attach({ text: () => made.text(), focused: () => made.focused, save: () => void save() });
    made.focus();
    return () => {
      diffEdit.detach();
      made.destroy();
      if (editor === made) editor = null;
    };
  });

  async function save(force = false) {
    const made = editor;
    if (!made) return;
    const doc = made.doc;
    if (await diffEdit.save(doc.toString(), force)) made.markSaved(doc);
    diffEdit.dirty = made.dirty;
  }
</script>

<div class="edit">
  <div class="bar">
    <span class="file truncate" title={opened?.path}>{opened?.path ?? ""}{diffEdit.dirty ? " *" : ""}</span>
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

  <EditBanners onkeep={() => void save(true)} />

  {#if diffEdit.refused}
    <div class="refused">
      <p>{diffEdit.refused}</p>
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

  .host {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  .host :global(.cm-editor) {
    min-height: 100%;
  }

  .refused {
    padding: var(--sp-5);
    color: var(--fg-secondary);
  }
</style>
