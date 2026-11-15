<script lang="ts">
  import { onMount } from "svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import Radio from "$components/common/Radio.svelte";
  import type { RepoId } from "$lib/ipc";
  import { clipboardPatch, patchPreview, readPatchFile, type PatchPreview } from "$lib/ipc/patches";
  import {
    defaultMode,
    mailSubjects,
    modeProblem,
    patchKind,
    statRows,
    type PatchMode,
  } from "$lib/patches";
  import { errors } from "$stores/errors.svelte";

  /** Apply Patch… (F-948): a patch from a file or the clipboard, checked and listed before
      anything is written. */
  interface Props {
    repo: RepoId;
    onapply: (text: string, mode: PatchMode) => void;
    onclose: () => void;
  }

  let { repo, onapply, onclose }: Props = $props();

  let text = $state("");
  let source = $state("");
  let mode = $state<PatchMode>("workTree");
  let preview = $state.raw<PatchPreview | null>(null);
  let checking = $state(false);
  let note = $state("");

  const kind = $derived(patchKind(text));
  const subjects = $derived(kind === "mailbox" ? mailSubjects(text) : []);
  const rows = $derived(statRows(preview?.files ?? []));
  const problem = $derived(text === "" ? null : (modeProblem(kind, mode) ?? preview?.problem ?? null));
  const ready = $derived(text !== "" && problem === null && preview !== null && !checking);

  function take(next: string, from: string) {
    text = next;
    source = from;
    note = "";
    mode = defaultMode(patchKind(next));
  }

  async function fromClipboard(quiet: boolean) {
    try {
      const found = await clipboardPatch();
      if (found) take(found, "Clipboard");
      else if (!quiet) note = "The clipboard holds no patch.";
    } catch (err) {
      errors.report(err, "Could not read the clipboard");
    }
  }

  async function fromFile() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const path = await open({
      title: "Apply Patch",
      filters: [
        { name: "Patch", extensions: ["patch", "diff", "eml", "mbox"] },
        { name: "All Files", extensions: ["*"] },
      ],
    });
    if (typeof path !== "string") return;
    try {
      take(await readPatchFile(path), path);
    } catch (err) {
      errors.report(err, "Could not read the patch");
    }
  }

  // Only the latest check counts: a slow one for the previous mode must not overwrite it.
  let asked = 0;
  $effect(() => {
    const [patch, how] = [text, mode];
    preview = null;
    if (patch === "" || modeProblem(patchKind(patch), how) !== null) return;
    const ticket = ++asked;
    checking = true;
    patchPreview(repo, patch, how)
      .then((found) => {
        if (ticket === asked) preview = found;
      })
      .catch((err: unknown) => errors.report(err, "Could not check the patch"))
      .finally(() => {
        if (ticket === asked) checking = false;
      });
  });

  onMount(() => void fromClipboard(true));

  function submit() {
    if (ready) onapply(text, mode);
  }
</script>

<Dialog title="Apply Patch" {onclose} onconfirm={submit} width="min(640px, 92vw)">
  <div class="form">
    <div class="source">
      <button type="button" class="btn" onclick={() => void fromFile()}>Choose File…</button>
      <button type="button" class="btn" onclick={() => void fromClipboard(false)}>From Clipboard</button>
      <span class="from mono" title={source}>{source || note || "No patch chosen"}</span>
    </div>

    {#if text !== ""}
      <div class="modes" role="radiogroup" aria-label="Apply">
        <Radio
          name="patch-mode"
          checked={mode === "mailbox"}
          disabled={kind !== "mailbox"}
          onchange={() => (mode = "mailbox")}
          label="As commits (git am --3way)"
        />
        <Radio
          name="patch-mode"
          checked={mode === "workTree"}
          onchange={() => (mode = "workTree")}
          label="To the working tree (git apply)"
        />
        <Radio
          name="patch-mode"
          checked={mode === "index"}
          onchange={() => (mode = "index")}
          label="To the index and the working tree (git apply --3way)"
        />
      </div>

      {#if subjects.length > 0 && mode === "mailbox"}
        <div class="list" aria-label="Commits">
          {#each subjects as subject, at (at)}
            <div class="row"><span class="grow">{subject}</span></div>
          {/each}
        </div>
      {/if}

      {#if rows.length > 0}
        <div class="list" aria-label="Files">
          {#each rows as row (row.label)}
            <div class="row">
              <span class="grow mono path" title={row.label}>{row.label}</span>
              <span class="counts mono" class:binary={row.binary}>{row.counts}</span>
            </div>
          {/each}
        </div>
      {/if}

      {#if problem}
        <pre class="problem mono">{problem}</pre>
      {:else if preview?.conflicts}
        <p class="caution">Applies with conflicts: resolve them, then commit or continue.</p>
      {/if}
    {/if}
  </div>

  {#snippet footer()}
    {#if checking}<span class="caption">Checking…</span>{/if}
    <span class="grow"></span>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="button" class="btn primary" disabled={!ready} onclick={submit}>Apply</button>
  {/snippet}
</Dialog>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
    font-size: var(--fs-dense);
    min-width: 0;
  }

  .source {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-width: 0;
  }

  .from {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-secondary);
  }

  .modes {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .list {
    max-height: 200px;
    overflow: auto;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
  }

  .row {
    display: flex;
    gap: var(--sp-3);
    padding: var(--sp-1) var(--sp-3);
  }

  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .counts {
    color: var(--text-secondary);
    white-space: nowrap;
  }

  .binary {
    font-style: italic;
  }

  .problem {
    margin: 0;
    max-height: 160px;
    overflow: auto;
    white-space: pre-wrap;
    color: var(--status-delete);
  }

  .caution,
  .caption {
    margin: 0;
    color: var(--text-secondary);
  }

  .grow {
    flex: 1 1 auto;
    min-width: 0;
  }
</style>
