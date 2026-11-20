<script lang="ts">
  import Checkbox from "$components/common/Checkbox.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import VirtualList from "$components/common/VirtualList.svelte";
  import DiffView from "$components/diff/DiffView.svelte";
  import { untrack } from "svelte";
  import { shortOid } from "$lib/format";
  import type { FileDiff, SplitFile, SplitPart } from "$lib/ipc";
  import {
    addPart,
    assign,
    blockKey,
    blocksOf,
    draftStarted,
    isAssigned,
    newDraft,
    ownerOf,
    partSizes,
    removePart,
    setActive,
    setMessage,
    splitCommitProblem,
    splitParts,
  } from "$lib/split-commit";
  import { errors } from "$stores/errors.svelte";
  import { uiMetricsStore } from "$stores/ui-metrics.svelte";

  interface Props {
    oid: string;
    message: string;
    files: readonly SplitFile[];
    published: boolean;
    busy: boolean;
    readDiff: (path: string) => Promise<FileDiff | null>;
    onsplit: (parts: SplitPart[]) => void;
    onclose: () => void;
  }

  let { oid, message, files, published, busy, readDiff, onsplit, onclose }: Props = $props();
  let draft = $state(untrack(() => newDraft(message)));
  let fileIndex = $state(0);
  let blockIndex = $state(0);
  let diff = $state.raw<FileDiff | null>(null);

  const blocks = $derived(blocksOf(files));
  const sizes = $derived(partSizes(draft, blocks));
  const problem = $derived(splitCommitProblem(draft, blocks));
  const runnable = $derived(problem === null && !busy);
  const file = $derived(files[fileIndex]);
  const last = $derived(draft.messages.length - 1);
  const rowHeight = $derived(uiMetricsStore.listRow);

  $effect(() => {
    diff = null;
    if (!file) return;
    let current = true;
    void readDiff(file.path).then(
      (loaded) => { if (current) diff = loaded; },
      (err: unknown) => { if (current) errors.report(err, "Could not preview the block"); },
    );
    return () => { current = false; };
  });

  const preview = $derived.by(() => {
    if (!file || !diff || file.whole || diff.kind !== "text") return diff;
    const hunk = diff.hunks[blockIndex];
    return hunk ? { ...diff, hunks: [hunk] } : null;
  });

  function split() {
    if (runnable) onsplit(splitParts(draft, blocks));
  }

  function close() {
    if (!busy) onclose();
  }
</script>

<Dialog
  title="Split Commit {shortOid(oid)}"
  onclose={close}
  onconfirm={split}
  dirty={!busy && draftStarted(draft, message)}
  width="min(1040px, 94vw)"
  height="min(760px, 92vh)"
  flush
>
  {#if published}
    <p class="warning">This commit is already on a remote. Splitting it requires a force-push.</p>
  {/if}

  <div class="draft">
    <div class="parts" role="group" aria-label="Commits">
      {#each draft.messages as _, part (part)}
        <button
          type="button"
          class="btn"
          aria-pressed={draft.active === part}
          disabled={busy}
          onclick={() => (draft = setActive(draft, part))}
        >
          Commit {part + 1} ({sizes[part] ?? 0})
        </button>
      {/each}
      <button type="button" class="btn" disabled={busy || draft.messages.length >= blocks.length}
        onclick={() => (draft = addPart(draft, message))}>Add Commit</button>
      <button type="button" class="btn" disabled={busy || draft.messages.length <= 2}
        onclick={() => (draft = removePart(draft, draft.active))}>Remove Commit {draft.active + 1}</button>
    </div>

    <label class="message">
      Message for commit {draft.active + 1}
      <textarea rows="3" disabled={busy} value={draft.messages[draft.active] ?? ""}
        oninput={(event) => (draft = setMessage(draft, draft.active, event.currentTarget.value))}
      ></textarea>
    </label>
    <p class="hint">Check blocks to assign them to commit {draft.active + 1}. Unassigned blocks go to commit {last + 1}.</p>
  </div>

  <div class="comparison">
    <div class="selection">
      <div class="list">
        <div class="caption">Files</div>
        {#snippet fileRow(item: SplitFile, at: number)}
          <button class="positioned list-row truncate" type="button" aria-current={at === fileIndex}
            style:top={`${at * rowHeight}px`} style:height={`${rowHeight}px`}
            onclick={() => { fileIndex = at; blockIndex = 0; }}>
            {item.path}
          </button>
        {/snippet}
        <VirtualList items={files} row={fileRow} />
      </div>
      {#if file}
        <div class="list">
          <div class="caption">Blocks{file.note ? ` · ${file.note}` : ""}</div>
          {#snippet blockRow(label: string, at: number)}
            {@const key = blockKey(file.path, file.whole ? null : at)}
            {@const owner = ownerOf(draft, key)}
            <div class="positioned assignment" style:top={`${at * rowHeight}px`} style:height={`${rowHeight}px`}>
              <Checkbox
                checked={isAssigned(draft, key) && owner === draft.active}
                disabled={busy}
                ariaLabel={`Assign ${label} to commit ${draft.active + 1}`}
                onchange={() => (draft = assign(draft, [key], draft.active))}
              />
              <button class="list-row block-label truncate" type="button" aria-current={at === blockIndex}
                onclick={() => (blockIndex = at)}>{label}</button>
              <span class="owner">Commit {owner + 1}{isAssigned(draft, key) ? "" : " (default)"}</span>
            </div>
          {/snippet}
          {#key file.path}
            <VirtualList items={file.blocks} row={blockRow} />
          {/key}
        </div>
      {/if}
    </div>
    <div class="preview">
      {#if file && preview}
        <DiffView diff={preview} path={file.path} active={false} hunkButtons={false} />
      {/if}
    </div>
  </div>

  {#snippet footer()}
    <span class="problem" role="status">{problem ?? ""}</span>
    <button type="button" class="btn" disabled={busy} onclick={close}>Cancel</button>
    <button type="button" class="btn primary" disabled={!runnable} onclick={split}>
      {busy ? "Splitting…" : `Split into ${draft.messages.length} Commits`}
    </button>
  {/snippet}
</Dialog>

<style>
  .warning {
    margin: 0;
    padding: var(--sp-3) var(--sp-5);
    background: var(--c-deleted-bg);
  }

  .draft {
    padding: var(--sp-4) var(--sp-5);
    overflow: auto;
    max-height: 48%;
    border-bottom: 1px solid var(--divider);
  }

  .parts {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }

  .parts button[aria-pressed="true"],
  .list-row[aria-current="true"] {
    background: var(--state-selected);
  }

  .message {
    display: grid;
    gap: var(--sp-2);
    margin-top: var(--sp-3);
  }

  .message textarea {
    resize: vertical;
    font-family: var(--font-mono);
  }

  .hint {
    margin: var(--sp-2) 0 0;
    color: var(--text-secondary);
  }

  .comparison,
  .selection,
  .list,
  .preview {
    display: flex;
    min-height: 0;
    min-width: 0;
  }

  .comparison,
  .list,
  .preview {
    flex: 1 1 0;
  }

  .selection,
  .list,
  .preview {
    flex-direction: column;
  }

  .selection {
    width: 38%;
    border-right: 1px solid var(--divider);
  }

  .list + .list {
    border-top: 1px solid var(--divider);
  }

  .caption {
    padding: var(--sp-2) var(--sp-4);
    color: var(--text-secondary);
    overflow-wrap: anywhere;
  }

  .list-row {
    display: block;
    width: 100%;
    height: 100%;
    padding: 0 var(--sp-4);
    border: 0;
    background: transparent;
    text-align: left;
    font-family: var(--font-mono);
  }

  .list-row:hover {
    background: var(--state-hover);
  }

  .positioned {
    position: absolute;
    inset-inline: 0;
  }

  .assignment {
    display: flex;
    align-items: center;
    height: 100%;
    padding-inline: var(--sp-4);
  }

  .block-label {
    flex: 1;
    min-width: 0;
  }

  .owner {
    font-size: var(--fs-small);
    color: var(--text-secondary);
  }

  .problem {
    flex: 1 1 auto;
    color: var(--status-modify);
  }

  @media (max-width: 700px) {
    .comparison {
      flex-direction: column;
    }

    .selection {
      flex: 1 1 0;
      width: 100%;
      border-right: 0;
      border-bottom: 1px solid var(--divider);
    }
  }
</style>
