<script lang="ts">
  import Radio from "$components/common/Radio.svelte";
  import RevisionCombobox from "$components/common/RevisionCombobox.svelte";
  import OptionRow from "$components/common/template/OptionRow.svelte";
  import TemplateDialog from "$components/common/template/TemplateDialog.svelte";
  import { shortDate } from "$lib/format";
  import {
    checkBranchName,
    checkRevision,
    worktreeFolderProblem,
    type Branch,
    type RepoId,
    type RevisionCheck,
    type Tag,
    type WorktreeBranch,
    type WorktreeEntry,
  } from "$lib/ipc";
  import { revisionOptions } from "$lib/revision-options";
  import {
    addProblem,
    addRequest,
    commandPreview,
    defaultBase,
    folderLabel,
    localNameOfRemote,
    newNameProblem,
    rememberOpenAfter,
    rememberedOpenAfter,
    remoteBase,
    suggestFolder,
    type AddChecks,
    type AddForm,
    type AddMode,
    type AddOrigin,
  } from "$lib/worktree-add";

  /** Add Worktree (item 13): Branch first, then Folder, over the dialog template. */
  interface Props {
    repo: RepoId;
    root: string;
    branches: readonly Branch[];
    tags: readonly Tag[];
    worktrees: readonly WorktreeEntry[];
    origin: AddOrigin;
    onbrowse: () => Promise<string | null>;
    onadd: (request: { path: string; branch: WorktreeBranch; open: boolean }) => void;
    onclose: () => void;
  }

  let { repo, root, branches, tags, worktrees, origin, onbrowse, onadd, onclose }: Props = $props();

  const CHECK_DELAY_MS = 200;
  const MODES: readonly (readonly [AddMode, string])[] = [
    ["new", "New branch"],
    ["existing", "Existing branch"],
    ["detached", "Detached at commit"],
  ];

  /* The dialog is modal: what it starts from is read once, in a closure. */
  const start = (() => {
    const held = new Map(
      worktrees.flatMap((entry) => (entry.branch ? [[entry.branch, entry.path] as const] : [])),
    );
    const current = branches.find((branch) => branch.isHead)?.name ?? null;
    const firstFree = branches.find((branch) => branch.kind === "local" && !held.has(branch.name))?.name ?? "";
    return {
      held,
      selectedCommit: origin.kind === "commit" ? origin.oid : null,
      base: defaultBase(origin, current),
      existing: origin.kind === "branch" && !held.has(origin.name) ? origin.name : firstFree,
    };
  })();
  const { held, selectedCommit } = start;

  let mode = $state<AddMode>("new");
  let name = $state("");
  let base = $state(start.base);
  let track = $state(true);
  let existing = $state(start.existing);
  let folder = $state("");
  let folderEdited = $state(false);
  let openAfter = $state(rememberedOpenAfter());

  let baseAnswer = $state<{ rev: string; check: RevisionCheck } | null>(null);
  let nameAnswer = $state<AddChecks["name"]>(null);
  let folderAnswer = $state<AddChecks["folder"]>(null);

  const form = $derived<AddForm>({ mode, name, base, track, existing, folder });
  const baseOptions = $derived(revisionOptions({ branches, tags, selectedCommit }));
  const branchOptions = $derived(
    revisionOptions({ branches, tags: [], special: false, held, localTwins: true }),
  );
  const remote = $derived(mode === "new" ? remoteBase(base, branches) : null);
  const checks = $derived<AddChecks>({
    base: baseAnswer ? { rev: baseAnswer.rev, problem: baseAnswer.check.problem } : null,
    name: nameAnswer,
    folder: folderAnswer,
  });
  const verdict = $derived(addProblem(form, branches, checks, held));
  const nameError = $derived(name.trim() === "" ? null : newNameProblem(name.trim(), branches, nameAnswer));
  const baseShown = $derived(baseAnswer?.rev === base.trim() ? baseAnswer.check : null);
  const suggested = $derived(suggestFolder(root, folderLabel(form, branches)));
  const command = $derived(commandPreview(form, branches, root));
  const tracking = $derived(
    mode === "existing" && branches.some((branch) => branch.kind === "remote" && branch.name === existing)
      ? `Creates the local branch ${localNameOfRemote(existing)} tracking ${existing}`
      : null,
  );

  $effect(() => {
    if (!folderEdited) folder = suggested;
  });

  /** Asks git after a pause; an answer that arrives after the input moved on is dropped. */
  function ask<T>(wanted: boolean, run: () => Promise<T>, keep: (answer: T) => void) {
    if (!wanted) return;
    let live = true;
    const timer = setTimeout(() => {
      run()
        .then((answer) => live && keep(answer))
        .catch((err: unknown) => console.error("check failed", err));
    }, CHECK_DELAY_MS);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }

  $effect(() => {
    const rev = base.trim();
    return ask(mode !== "existing" && rev !== "", () => checkRevision(repo, rev), (check) => (baseAnswer = { rev, check }));
  });

  $effect(() => {
    const wanted = name.trim();
    return ask(
      mode === "new" && wanted !== "",
      () => checkBranchName(repo, wanted),
      (problem) => (nameAnswer = { name: wanted, problem }),
    );
  });

  $effect(() => {
    const path = folder.trim().replace(/\\/g, "/").replace(/\/+$/, "");
    return ask(path !== "", () => worktreeFolderProblem(path), (problem) => (folderAnswer = { path, problem }));
  });

  function submit() {
    if (verdict.text !== null || verdict.pending) return;
    rememberOpenAfter(openAfter);
    const { path, branch } = addRequest(form, branches);
    onadd({ path, branch, open: openAfter });
  }

  async function browse() {
    const picked = await onbrowse();
    if (picked) {
      folder = picked.replace(/\\/g, "/");
      folderEdited = true;
    }
  }

  const ago = (seconds: number) => shortDate(seconds, -new Date().getTimezoneOffset());
</script>

{#snippet preview()}
  <div class="preview" class:bad={baseShown?.problem}>
    {#if base.trim() === ""}
      <!-- Nothing to say yet; the footer asks for a base. -->
    {:else if baseShown === null}
      Checking…
    {:else if baseShown.problem}
      {baseShown.problem}
    {:else if baseShown.commit}
      <span class="mono">{baseShown.commit.shortOid}</span>
      {baseShown.commit.subject}
      <span class="when">· {ago(baseShown.commit.date)}</span>
    {/if}
  </div>
{/snippet}

<TemplateDialog
  title="Add Worktree"
  {onclose}
  status={verdict.text}
  actions={[
    {
      label: "Add",
      primary: true,
      disabled: verdict.text !== null || verdict.pending,
      tip: verdict.pending && verdict.text === null ? "Waiting for the checks to finish" : undefined,
      onclick: submit,
    },
  ]}
>
  <p class="sub">Check out a branch or commit in a separate folder, next to this repository.</p>

  <div class="modes" role="radiogroup" aria-label="Branch">
    {#each MODES as [key, title] (key)}
      <Radio name="worktree-mode" checked={mode === key} label={title} onchange={() => (mode = key)} />
    {/each}
  </div>

  {#if mode === "new"}
    <div class="field">
      <span class="lbl">Name</span>
      <input type="text" data-autofocus bind:value={name} placeholder="feature/…" spellcheck="false" />
      {#if nameError}<div class="preview bad">{nameError}</div>{/if}
    </div>
    <div class="field">
      <span class="lbl">Based on</span>
      <RevisionCombobox bind:value={base} options={baseOptions} label="Based on" placeholder="Branch, tag, hash or HEAD~2" />
      {@render preview()}
    </div>
    {#if remote}
      <OptionRow bind:checked={track} label={`Track ${remote}`} hint="Pull and push use it as the upstream (--track)" />
    {/if}
  {:else if mode === "existing"}
    <div class="field">
      <span class="lbl">Branch</span>
      <RevisionCombobox bind:value={existing} options={branchOptions} label="Branch" free={false} placeholder="Choose a branch" />
      {#if tracking}<div class="preview">{tracking}</div>{/if}
    </div>
  {:else}
    <div class="field">
      <span class="lbl">Commit</span>
      <RevisionCombobox bind:value={base} options={baseOptions} label="Commit" placeholder="Branch, tag, hash or HEAD~2" />
      {@render preview()}
    </div>
  {/if}

  <div class="field">
    <span class="lbl">Folder</span>
    <span class="with-button">
      <input
        type="text"
        bind:value={folder}
        oninput={(event) => (folderEdited = event.currentTarget.value !== "")}
        placeholder="Where the new checkout goes"
        spellcheck="false"
        aria-label="Folder"
      />
      <button type="button" class="btn" onclick={() => void browse()}>Browse…</button>
    </span>
  </div>

  <OptionRow bind:checked={openAfter} label="Open worktree after creation" />

  <div class="command" title="The command that will run">{command}</div>
</TemplateDialog>

<style>
  .modes {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-3) var(--sp-6);
    font-size: var(--fs-dense);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
    font-size: var(--fs-dense);
  }

  .lbl {
    color: var(--text-secondary);
  }

  .with-button {
    display: flex;
    gap: var(--sp-3);
  }

  .with-button input {
    flex: 1 1 auto;
    min-width: 0;
  }

  .preview {
    color: var(--text-muted);
    line-height: 1.4;
    overflow-wrap: anywhere;
  }

  .preview.bad {
    color: var(--status-danger);
  }

  .preview .mono {
    font-family: var(--font-mono);
  }

  .command {
    padding: var(--sp-3) var(--sp-4);
    background: var(--bg-panel);
    border-radius: var(--r-sm);
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: var(--fs-dense);
    line-height: 1.4;
    overflow-wrap: anywhere;
    user-select: text;
  }
</style>
