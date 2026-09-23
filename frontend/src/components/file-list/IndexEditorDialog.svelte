<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import { editedSides, editorSide } from "$lib/file-dialogs";
  import type { IndexEditorSides } from "$lib/ipc/file-menus";

  /** Three versions of one file side by side (#40): HEAD to read, the index and the working
      tree to edit. Save writes only the panes that changed; the index takes the text as
      it is, with no line-ending conversion. */
  interface Props {
    path: string;
    sides: IndexEditorSides;
    saving: boolean;
    onsave: (edited: { index: string | null; worktree: string | null }) => void;
    onclose: () => void;
  }

  let { path, sides, saving, onsave, onclose }: Props = $props();

  const head = $derived(editorSide(sides.head));
  const indexSide = $derived(editorSide(sides.index));
  const worktreeSide = $derived(editorSide(sides.worktree));

  // svelte-ignore state_referenced_locally
  let indexText = $state(editorSide(sides.index).text);
  // svelte-ignore state_referenced_locally
  let worktreeText = $state(editorSide(sides.worktree).text);

  const edited = $derived(
    editedSides({ index: indexSide, worktree: worktreeSide }, { index: indexText, worktree: worktreeText }),
  );
  const dirty = $derived(edited.index !== null || edited.worktree !== null);

  function note(text: string, against: string, present: boolean, label: string): string {
    if (!present && text === "") return "absent";
    return text === against ? `same as ${label}` : `differs from ${label}`;
  }

  function submit() {
    if (dirty && !saving) onsave(edited);
  }
</script>

<Dialog title="Index Editor — {path}" {onclose} width="min(1280px, 96vw)" height="min(720px, 90vh)">
  {#if sides.binary}
    <p class="binary">{path} is not a text file; the Index Editor edits text only.</p>
  {:else}
    <div class="panes">
      <section>
        <header>
          <strong>HEAD</strong>
          <span class="note">{head.present ? "read-only" : "absent"}</span>
        </header>
        <textarea class="mono" readonly spellcheck="false" value={head.text} aria-label="HEAD version"></textarea>
      </section>
      <section>
        <header>
          <strong>Index</strong>
          <span class="note">{note(indexText, head.text, indexSide.present, "HEAD")}</span>
          <span class="grow"></span>
          <button type="button" class="btn" title="Put the HEAD version in the index pane" onclick={() => (indexText = head.text)}>← HEAD</button>
          <button type="button" class="btn" title="Put the working tree version in the index pane" onclick={() => (indexText = worktreeText)}>Working Tree →</button>
        </header>
        <textarea class="mono" spellcheck="false" bind:value={indexText} aria-label="Index version"></textarea>
      </section>
      <section>
        <header>
          <strong>Working Tree</strong>
          <span class="note">{note(worktreeText, indexText, worktreeSide.present, "Index")}</span>
          <span class="grow"></span>
          <button type="button" class="btn" title="Put the index version in the working tree pane" onclick={() => (worktreeText = indexText)}>← Index</button>
        </header>
        <textarea class="mono" spellcheck="false" bind:value={worktreeText} aria-label="Working tree version"></textarea>
      </section>
    </div>
  {/if}

  {#snippet footer()}
    {#if !sides.binary}<span class="hint">Save writes only the panes you changed.</span>{/if}
    <span class="grow"></span>
    <button type="button" class="btn" onclick={onclose}>{sides.binary ? "Close" : "Cancel"}</button>
    {#if !sides.binary}
      <button type="button" class="btn primary" disabled={!dirty || saving} onclick={submit}>
        {saving ? "Saving…" : "Save"}
      </button>
    {/if}
  {/snippet}
</Dialog>

<style>
  .panes {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-3);
    flex: 1 1 auto;
    min-height: 0;
    height: 100%;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
    min-height: 0;
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-height: 26px;
    font-size: var(--fs-dense);
  }

  .note,
  .hint {
    color: var(--text-secondary);
    font-size: 11px;
  }

  textarea {
    flex: 1 1 auto;
    min-height: 0;
    resize: none;
    padding: var(--sp-2) var(--sp-3);
    white-space: pre;
    overflow: auto;
    color: var(--text-primary);
    background: var(--surface-input);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    tab-size: 4;
  }

  textarea[readonly] {
    color: var(--text-secondary);
  }

  .binary {
    margin: 0;
    font-size: var(--fs-dense);
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
