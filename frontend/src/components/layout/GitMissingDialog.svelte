<script lang="ts">
  import Button from "$components/common/Button.svelte";
  import Callout from "$components/common/template/Callout.svelte";
  import TemplateDialog from "$components/common/template/TemplateDialog.svelte";
  import { ON_WINDOWS } from "$lib/git-check";
  import { GIT_DOWNLOAD_URL, WHY_GIT, installHint, oldGitText, osOf } from "$lib/git-missing";
  import type { GitMissingStore } from "$stores/git-missing.svelte";

  /** The missing-git dialog (R-700): why, what was found, and the three ways out. */
  interface Props {
    store: GitMissingStore;
  }

  let { store }: Props = $props();

  const dialog = $derived(store.open);
  const hint = installHint(osOf(typeof navigator === "undefined" ? "" : navigator.platform));

  async function browse() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    // An extension filter cannot match the extensionless `git` of Unix, so only Windows gets one.
    const picked = await open({
      title: "Select Git Executable",
      multiple: false,
      directory: false,
      filters: ON_WINDOWS ? [{ name: "Git (git.exe)", extensions: ["exe"] }] : undefined,
    });
    if (typeof picked === "string") void store.choose(picked);
  }

  async function download() {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(GIT_DOWNLOAD_URL);
  }
</script>

{#if dialog?.kind === "missing"}
  <TemplateDialog
    title="Git was not found"
    cancelLabel="Continue without git"
    onclose={() => store.continueWithout()}
    actions={[
      { label: "Download Git", onclick: () => void download(), tip: GIT_DOWNLOAD_URL },
      { label: "Choose git executable…", onclick: () => void browse(), primary: true },
    ]}
  >
    <p class="sub">{WHY_GIT} Without it you can still browse repositories, but nothing can be changed.</p>

    {#if dialog.reason}
      <Callout kind="danger"><span class="reason mono" title={dialog.reason}>{dialog.reason}</span></Callout>
    {/if}

    {#if store.picked}
      <div class="verdict {store.picked.state}" role="status">
        {#if store.picked.state === "ok"}
          <span aria-hidden="true">✓</span> git {store.picked.version} — {store.picked.path}
        {:else if store.picked.state === "bad"}
          <span aria-hidden="true">✗</span> {store.picked.path}: {store.picked.reason}
        {:else}
          Checking {store.picked.path}…
        {/if}
      </div>
    {/if}

    <div class="found">
      <p class="sub">Found on this computer</p>
      {#if store.candidates === null}
        <p class="sub">Looking…</p>
      {:else if store.candidates.length === 0}
        <p class="sub">No working git in the usual places. Install it, or choose the file yourself.</p>
      {:else}
        <ul>
          {#each store.candidates as candidate (candidate.path)}
            <li>
              <span class="path mono truncate" title={candidate.path}>{candidate.path}</span>
              <span class="version">{candidate.version}</span>
              <Button size="sm" onclick={() => void store.choose(candidate.path)}>Use</Button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <p class="sub">To install: <code class="mono">{hint}</code></p>
  </TemplateDialog>
{:else if dialog?.kind === "old"}
  <TemplateDialog
    title="Git is older than recommended"
    cancelLabel="Close"
    onclose={() => store.continueWithout()}
    actions={[{ label: "Download Git", onclick: () => void download(), tip: GIT_DOWNLOAD_URL, primary: true }]}
  >
    <Callout>{oldGitText(dialog.version, dialog.minimum)}</Callout>
  </TemplateDialog>
{/if}

<style>
  .reason {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .verdict {
    font-size: var(--fs-dense);
    line-height: 1.4;
    overflow-wrap: anywhere;
  }

  .verdict.ok {
    color: var(--status-success);
  }

  .verdict.bad {
    color: var(--status-danger);
  }

  .verdict.checking {
    color: var(--text-secondary);
  }

  .found {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
  }

  .found .sub {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  li {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    min-width: 0;
    padding: var(--sp-2) var(--sp-4);
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
  }

  .path {
    flex: 1 1 auto;
    min-width: 0;
    font-size: var(--fs-dense);
    color: var(--text-primary);
  }

  .version {
    flex: none;
    font-size: var(--fs-dense);
    color: var(--text-secondary);
  }

  code {
    color: var(--text-primary);
    overflow-wrap: anywhere;
  }
</style>
