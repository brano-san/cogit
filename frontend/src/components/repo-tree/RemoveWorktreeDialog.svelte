<script lang="ts">
  import FileStateIcon from "$components/common/FileStateIcon.svelte";
  import SkeletonRows from "$components/common/SkeletonRows.svelte";
  import Callout from "$components/common/template/Callout.svelte";
  import ObjectCard, { type CardRow } from "$components/common/template/ObjectCard.svelte";
  import OptionRow from "$components/common/template/OptionRow.svelte";
  import TemplateDialog from "$components/common/template/TemplateDialog.svelte";
  import { fileState } from "$lib/file-state";
  import { shortOid } from "$lib/format";
  import type { WorktreeEntry } from "$lib/ipc";
  import { removalNeeds } from "$lib/worktree-list";
  import {
    applyChunk,
    removeButton,
    scanSettled,
    stageRows,
    startScan,
    uncommittedList,
  } from "$lib/worktree-scan";
  import { worktrees } from "$stores/worktrees.svelte";

  /** Removing a worktree with work or submodules in it needs a second, separate yes: what
      is at stake, and a box for `--force` (R-184, R-434). The dialog reads that itself, in
      three parallel stages, and stops reading when it closes (R-675). */
  interface Props {
    entry: WorktreeEntry;
    onremove: (force: boolean) => void;
    onclose: () => void;
  }

  let { entry, onremove, onclose }: Props = $props();

  const SHOWN = 40;
  const COMMITS = 3;
  let force = $state(false);
  let scan = $state.raw(startScan());

  $effect(() => worktrees.scanRemoval(entry.path, (chunk) => (scan = applyChunk(scan, chunk))));

  const settled = $derived(scanSettled(scan));
  const list = $derived(uncommittedList(scan));
  const stages = $derived(stageRows(scan));
  const needs = $derived(removalNeeds(entry, list));
  // The list's `dirty` mark is the cached answer: it keeps the dialog its size until the read lands.
  const expectChanges = $derived(list === null ? entry.dirty : list.length > 0);
  const hasSubmodules = $derived(
    entry.hasSubmodules || (scan.submodules.status === "done" && scan.submodules.value.paths.length > 0),
  );
  const unpushed = $derived(scan.unpushed.status === "done" ? scan.unpushed.value : []);
  const button = $derived(removeButton(entry, scan, force));
  const showForce = $derived(settled ? needs.force : entry.dirty || entry.hasSubmodules || needs.locked);

  const rows = $derived.by(() => {
    const list: CardRow[] = [{ label: "Worktree", value: entry.name }];
    if (entry.branch) list.push({ label: "Branch", value: entry.branch, mono: true });
    list.push({ label: "Path", value: entry.path, mono: true });
    return list;
  });

  function submit() {
    if (!button.disabled) onremove(needs.force);
  }
</script>

<TemplateDialog
  title="Remove Worktree"
  {onclose}
  destructive
  actions={[{ label: button.label, primary: true, disabled: button.disabled, tip: button.tip, onclick: submit }]}
>
  <p class="sub">The worktree is unregistered and its folder is deleted</p>
  <ObjectCard {rows} />

  <section class="block" aria-busy={!settled}>
    <ul class="stages">
      {#each stages as stage (stage.id)}
        <li class={stage.status}>
          <span class="glyph" aria-hidden="true"></span>
          <span class="label">{stage.label}</span>
          <span class="detail truncate" title={stage.detail}>{stage.detail}</span>
        </li>
      {/each}
    </ul>

    {#if list === null}
      <div class="checking" class:tall={entry.dirty}>
        <SkeletonRows rows={entry.dirty ? 4 : 1} />
        <p class="sub caption">Checking uncommitted changes…</p>
      </div>
    {:else if list.length > 0}
      <h4>Uncommitted changes ({list.length})</h4>
      <ul class="changes">
        {#each list.slice(0, SHOWN) as file (file.path)}
          {@const state = fileState(file, "worktree")}
          <li>
            <FileStateIcon base={file.mode === "submodule" ? "repository" : "page"} state={state.icon} />
            <span class="mono truncate">{file.path}</span>
            <span class="state truncate" title={state.tooltip}>{state.text}</span>
          </li>
        {/each}
        {#if list.length > SHOWN}<li class="more">and {list.length - SHOWN} more</li>{/if}
      </ul>
    {/if}
  </section>

  {#if expectChanges}
    <Callout kind="warning">
      {#if list === null}This worktree has uncommitted changes.{:else}{list.length === 1
          ? "1 uncommitted change"
          : `${list.length} uncommitted changes`} will be lost from the folder.{/if}
    </Callout>
  {/if}

  {#if needs.locked}
    <Callout kind="warning">
      This worktree is locked{entry.locked ? `: ${entry.locked}` : ""}. Removing it needs --force, which
      gets past the lock.
    </Callout>
  {/if}

  {#if hasSubmodules}
    <Callout kind="danger">
      Submodules are checked out in this worktree. Removing it needs --force and deletes their
      repositories too.
    </Callout>
  {/if}

  {#if unpushed.length > 0}
    <Callout kind="danger">
      <div>Commits no remote has will be lost. A stash does not save them:</div>
      <ul class="unpushed">
        {#each unpushed as item (item.path)}
          <li>
            <span class="mono">{item.path}</span>
            <span>{item.total === 1 ? "1 commit" : `${item.total} commits`}</span>
            <ul class="commits">
              {#each item.commits.slice(0, COMMITS) as commit (commit.oid)}
                <li class="truncate"><span class="mono">{shortOid(commit.oid)}</span> {commit.summary}</li>
              {/each}
              {#if item.total > COMMITS}<li>and {item.total - COMMITS} more</li>{/if}
            </ul>
          </li>
        {/each}
      </ul>
    </Callout>
  {/if}

  {#if showForce}
    <OptionRow
      bind:checked={force}
      disabled={!settled}
      label="Remove anyway (--force)"
      hint={expectChanges
        ? "Changes are stashed first; Undo brings them back"
        : "Git refuses to remove a worktree with submodules without it"}
    />
  {/if}
</TemplateDialog>

<style>
  .block {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
  }

  h4 {
    margin: 0;
    font-size: var(--fs-dense);
    font-weight: 600;
    color: var(--text-primary);
  }

  .stages {
    display: grid;
    grid-template-columns: max-content max-content minmax(0, 1fr);
    gap: var(--sp-2) var(--sp-4);
    align-items: center;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-dense);
  }

  .stages li {
    display: contents;
  }

  .label {
    color: var(--text-primary);
  }

  .detail {
    color: var(--text-secondary);
  }

  .failed .detail {
    color: var(--status-danger);
  }

  .glyph {
    box-sizing: border-box;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 2px solid var(--divider);
  }

  .running .glyph {
    border-top-color: var(--accent);
    animation: spin 0.9s linear infinite;
  }

  .done .glyph {
    border-color: var(--status-add);
    background: var(--status-add);
  }

  .failed .glyph {
    border-color: var(--status-danger);
    background: var(--status-danger);
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .running .glyph {
      animation: none;
    }
  }

  .checking {
    padding: var(--sp-2) 0;
    background: var(--bg-panel);
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
  }

  .checking.tall {
    min-height: 8em;
  }

  .caption {
    padding: 0 var(--sp-5);
  }

  .changes {
    max-height: 12em;
    margin: 0;
    padding: var(--sp-2) var(--sp-3);
    overflow-y: auto;
    list-style: none;
    background: var(--surface-input);
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
  }

  .changes li {
    display: flex;
    gap: var(--sp-3);
    min-width: 0;
  }

  .state,
  .more {
    color: var(--text-secondary);
  }

  .state {
    margin-left: auto;
    padding-left: var(--sp-4);
  }

  .unpushed,
  .commits {
    margin: var(--sp-2) 0 0;
    padding: 0;
    list-style: none;
  }

  .commits {
    margin: 0 0 var(--sp-2) var(--sp-5);
    color: var(--text-secondary);
  }

  .unpushed > li {
    display: flex;
    flex-wrap: wrap;
    gap: 0 var(--sp-4);
  }

  .unpushed .commits {
    flex-basis: 100%;
  }
</style>
