<script lang="ts">
  import { tick } from "svelte";
  import Checkbox from "$components/common/Checkbox.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import Select from "$components/common/Select.svelte";
  import { CLONE_PAGES, PAGE_TITLES, joinPath } from "$lib/clone";
  import type { CloneRequest } from "$lib/ipc/clone";
  import type { CloneWizard } from "$stores/clone.svelte";

  /** Repository ▸ Clone…: Repository, Selection, Directory, as in SmartGit (F-575). */
  interface Props {
    wizard: CloneWizard;
    onbrowse: (title: string) => Promise<string | null>;
    onfinish: (request: CloneRequest) => void;
    onclose: () => void;
  }

  let { wizard, onbrowse, onfinish, onclose }: Props = $props();

  let form: HTMLDivElement | undefined = $state();
  const last = $derived(wizard.page === "directory");

  // The field that had the focus is gone with its page; the next page's first one takes it.
  $effect(() => {
    void wizard.page;
    void tick().then(() => form?.querySelector<HTMLElement>("input:not(:disabled)")?.focus());
  });
  /** Git's own words: the output in full, never a summary instead of it. */
  const failure = $derived.by(() => {
    const error = wizard.failed;
    if (error === null) return null;
    if (error.detail.kind !== "command") return error.message;
    const { stderr, stdout } = error.detail.data;
    return stderr.trim() || stdout.trim() || error.message;
  });

  function confirm() {
    if (!last) {
      void wizard.next();
      return;
    }
    const request = wizard.finish();
    if (request) onfinish(request);
  }

  async function browseSource() {
    const picked = await onbrowse("Repository to Clone");
    if (picked) wizard.setSource(picked);
  }

  async function browseParent() {
    const picked = await onbrowse("Folder to Clone Into");
    if (picked) wizard.setParent(picked);
  }
</script>

<Dialog title="Clone" {onclose} onconfirm={confirm} dirty={wizard.dirty} width="min(560px, 92vw)">
  <div class="form" bind:this={form}>
    <ol class="steps">
      {#each CLONE_PAGES as page, index (page)}
        <li class:current={wizard.page === page}>{index + 1}. {PAGE_TITLES[page]}</li>
      {/each}
    </ol>

    {#if wizard.page === "repository"}
      <label class="field">
        <span>Repository URL or local folder</span>
        <span class="with-button">
          <input
            type="text"
            class="mono"
            spellcheck="false"
            value={wizard.source}
            placeholder="https://host/owner/name.git, git@host:owner/name.git or D:\repos\name"
            data-autofocus
            oninput={(event) => wizard.setSource(event.currentTarget.value)}
          />
          <button type="button" class="btn" onclick={() => void browseSource()}>Browse…</button>
        </span>
        <span class="hint">Next checks that the repository can be reached. The check asks for no password; stored credentials still answer.</span>
      </label>
      {#if failure !== null}
        <div class="failure" role="alert">
          <strong>The repository could not be reached.</strong>
          Change the URL, or continue without the check to sign in during the clone.
          <pre class="mono">{failure}</pre>
        </div>
      {/if}
    {:else if wizard.page === "selection"}
      <Checkbox checked={wizard.submodules} label="Include submodules" onchange={(on) => (wizard.submodules = on)} />
      <Checkbox
        checked={wizard.allBranches}
        label="Fetch all heads and tags"
        onchange={(on) => (wizard.allBranches = on)}
      />
      <div class="nested">
        <Checkbox
          checked={wizard.skipLarge}
          label="Skip large files (partial clone)"
          onchange={(on) => (wizard.skipLarge = on)}
        />
        <label class="limit" class:disabled={!wizard.skipLarge}>
          <span>Omit files larger than</span>
          <input
            type="text"
            class="size"
            inputmode="numeric"
            value={wizard.limitMb}
            disabled={!wizard.skipLarge}
            title={wizard.skipLarge ? undefined : "Tick Skip large files first"}
            oninput={(event) => (wizard.limitMb = event.currentTarget.value)}
          />
          <span>MB</span>
        </label>
        <span class="hint">The files being checked out are always downloaded; larger ones elsewhere in the history stay on the server until a command needs them.</span>
      </div>
      <div class="field">
        <span>Check out branch</span>
        <Select
          value={wizard.branch ?? ""}
          label="Check out branch"
          options={wizard.branches.length > 0 ? wizard.branches : [["", "The server's default branch"]]}
          disabled={wizard.branchReason !== null}
          onchange={(next) => (wizard.branch = next)}
        />
        {#if wizard.branchReason}
          <span class="hint">{wizard.branchReason}</span>
        {:else if !wizard.allBranches}
          <span class="hint">Only this branch is fetched.</span>
        {/if}
      </div>
    {:else}
      <label class="field">
        <span>Parent folder</span>
        <span class="with-button">
          <input
            type="text"
            class="mono"
            spellcheck="false"
            value={wizard.parent}
            data-autofocus
            oninput={(event) => wizard.setParent(event.currentTarget.value)}
          />
          <button type="button" class="btn" onclick={() => void browseParent()}>Browse…</button>
        </span>
      </label>
      <label class="field">
        <span>Folder name</span>
        <input
          type="text"
          class="mono"
          spellcheck="false"
          value={wizard.name}
          oninput={(event) => wizard.setName(event.currentTarget.value)}
        />
      </label>
      <p class="target">
        Clone into <span class="mono">{joinPath(wizard.parent, wizard.name.trim())}</span>
      </p>
    {/if}
  </div>

  {#snippet footer()}
    {#if wizard.problem}<span class="problem">{wizard.problem}</span>{/if}
    <span class="grow"></span>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button
      type="button"
      class="btn"
      disabled={wizard.page === "repository"}
      title={wizard.page === "repository" ? "This is the first page" : undefined}
      onclick={() => wizard.back()}>Back</button
    >
    {#if wizard.failed !== null}
      <button type="button" class="btn" onclick={() => wizard.continueUnchecked()}>Continue Without Check</button>
    {/if}
    <button
      type="button"
      class="btn primary"
      disabled={wizard.problem !== null}
      title={wizard.problem ?? undefined}
      onclick={confirm}>{last ? "Finish" : "Next"}</button
    >
  {/snippet}
</Dialog>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
    font-size: var(--fs-dense);
  }

  .steps {
    display: flex;
    gap: var(--sp-6);
    margin: 0;
    padding: 0;
    list-style: none;
    color: var(--text-secondary);
  }

  .steps .current {
    color: var(--text-primary);
    font-weight: 600;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .with-button {
    display: flex;
    gap: var(--sp-3);
  }

  .with-button input {
    flex: 1 1 auto;
    min-width: 0;
  }

  .nested {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }

  .limit {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding-left: var(--sp-7);
  }

  .limit.disabled {
    color: var(--text-secondary);
  }

  .limit .size {
    width: 72px;
  }

  .hint {
    color: var(--text-secondary);
    line-height: 1.45;
  }

  .nested .hint {
    padding-left: var(--sp-7);
  }

  .failure {
    padding: var(--sp-3) var(--sp-4);
    border-left: 3px solid var(--status-delete);
    background: var(--surface-raised);
    border-radius: var(--r-sm);
    line-height: 1.45;
  }

  .failure pre {
    max-height: 160px;
    margin: var(--sp-2) 0 0;
    overflow: auto;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
  }

  .target {
    margin: 0;
    color: var(--text-secondary);
    overflow-wrap: anywhere;
  }

  .target .mono {
    color: var(--text-primary);
  }

  .problem {
    color: var(--status-modify);
    font-size: var(--fs-dense);
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
