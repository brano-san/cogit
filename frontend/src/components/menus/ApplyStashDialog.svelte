<script lang="ts">
  import Checkbox from "$components/common/Checkbox.svelte";
  import ObjectCard, { type CardRow } from "$components/common/template/ObjectCard.svelte";
  import OptionRow from "$components/common/template/OptionRow.svelte";
  import TemplateDialog from "$components/common/template/TemplateDialog.svelte";
  import { parseStashMessage } from "$lib/dialog-template";
  import { dateTooltip, shortOid } from "$lib/format";
  import { stashContents, toCogitError, type RepoId } from "$lib/ipc";
  import { pickedPaths, stashFiles, stashFileTag, type StashFile } from "$lib/stash-pick";
  import { stashes } from "$stores/stashes.svelte";

  /** Apply Stash (item 40): a double click on a stash and the menu's Apply Stash. Its file
      list picks what to apply (F-945); `paths` is `null` when every file is ticked. */
  interface Props {
    repo: RepoId;
    index: number;
    oid: string;
    message: string;
    onapply: (drop: boolean, restoreIndex: boolean, paths: string[] | null) => void;
    onclose: () => void;
  }

  let { repo, index, oid, message, onapply, onclose }: Props = $props();

  let restoreIndex = $state(false);
  let files = $state.raw<StashFile[] | null>(null);
  let failed = $state<string | null>(null);
  let unticked = $state.raw<ReadonlySet<string>>(new Set());

  $effect(() => {
    let live = true;
    stashContents(repo, index)
      .then((contents) => {
        if (!live) return;
        // The list moved while the dialog opened: those are another stash's files.
        if (contents.worktreeRev !== oid) failed = "The stash list changed; only the whole stash can be applied.";
        else files = stashFiles(contents);
      })
      .catch((err: unknown) => {
        if (live) failed = `Could not read the stash's files: ${toCogitError(err).message}`;
      });
    return () => {
      live = false;
    };
  });

  const paths = $derived(files ? pickedPaths(files, unticked) : null);
  const partial = $derived(paths !== null);
  const nothing = $derived(paths !== null && paths.length === 0);

  function toggle(path: string, checked: boolean) {
    const next = new Set(unticked);
    if (checked) next.delete(path);
    else next.add(path);
    unticked = next;
  }

  function toggleAll(checked: boolean) {
    unticked = checked ? new Set() : new Set((files ?? []).map((file) => file.path));
  }

  const rows = $derived.by(() => {
    const entry = stashes.entries.find((stash) => stash.index === index);
    const parsed = parseStashMessage(message);
    const list: CardRow[] = [{ label: "Stash", value: `stash@{${index}}`, mono: true }];
    if (parsed.branch) list.push({ label: "Branch", value: parsed.branch, mono: true });
    if (parsed.message) list.push({ label: "Message", value: parsed.message, clamp: 3 });
    if (entry) {
      list.push({ label: "Commit", value: shortOid(entry.oid), mono: true });
      list.push({ label: "Date", value: dateTooltip(entry.timestamp, -new Date().getTimezoneOffset()), mono: true });
    }
    return list;
  });
</script>

<TemplateDialog
  title={`Apply stash@{${index}}`}
  {onclose}
  fill
  status={nothing ? "Choose at least one file." : null}
  actions={[
    { label: "Apply", primary: partial, disabled: nothing, onclick: () => onapply(false, restoreIndex, paths) },
    {
      label: "Apply & Drop",
      primary: !partial,
      disabled: partial,
      tip: partial
        ? "Only the whole stash can be dropped: the stash keeps the files not applied"
        : "Removes the stash if it applied without conflicts",
      onclick: () => onapply(true, restoreIndex, null),
    },
  ]}
>
  <p class="sub">Changes will be applied to the working tree</p>
  <ObjectCard {rows} />
  {#if files && files.length > 0}
    <div class="files">
      <div class="row head">
        <Checkbox wide checked={unticked.size === 0} onchange={toggleAll}>
          <span>Files ({files.length - unticked.size} of {files.length})</span>
        </Checkbox>
      </div>
      {#each files as file (file.path)}
        <div class="row">
          <Checkbox wide checked={!unticked.has(file.path)} onchange={(checked) => toggle(file.path, checked)}>
            <span class="tag" title={file.status}>{stashFileTag(file.status)}</span>
            <span class="name" title={file.oldPath ? `${file.oldPath} → ${file.path}` : file.path}>{file.path}</span>
          </Checkbox>
        </div>
      {/each}
    </div>
  {:else if failed}
    <p class="sub">{failed}</p>
  {/if}
  <OptionRow bind:checked={restoreIndex} label="Restore Index" hint="Also restore what was staged (--index)" />
</TemplateDialog>

<style>
  /* Scrolls on its own; the card and the option stay put (`fill`). */
  .files {
    flex: 0 1 auto;
    min-height: 0;
    max-height: 240px;
    overflow-y: auto;
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
  }

  .row {
    display: flex;
    align-items: center;
    height: var(--h-row-dense);
    padding: 0 var(--sp-4);
    font-family: var(--font-mono);
    font-size: var(--fs-dense);
  }

  .row.head {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--bg-elevated);
    border-bottom: 1px solid var(--divider);
    font-family: inherit;
  }

  .row:not(.head):hover {
    background: var(--state-hover);
  }

  .tag {
    flex: 0 0 1.5ch;
    color: var(--text-secondary);
  }

  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
