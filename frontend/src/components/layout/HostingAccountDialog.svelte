<script lang="ts">
  import Callout from "$components/common/template/Callout.svelte";
  import OptionRow from "$components/common/template/OptionRow.svelte";
  import TemplateDialog from "$components/common/template/TemplateDialog.svelte";
  import type { HostingProvider } from "$lib/ipc";
  import { PROVIDERS, draftProblem, newAccount, selfHosted, serverPlaceholder, tokenHint } from "$lib/hosting";
  import { hosting } from "$stores/hosting.svelte";

  let provider = $state<HostingProvider>("github");
  let server = $state("");
  let email = $state("");
  let token = $state("");
  let useForGit = $state(true);
  let shown = $state(false);

  $effect.pre(() => {
    if (!hosting.adding) return;
    provider = "github";
    server = "";
    email = "";
    token = "";
    useForGit = true;
    shown = false;
  });

  const draft = $derived({ provider, server, email, token, useForGit });
  const problem = $derived(draftProblem(draft));

  function submit() {
    if (problem !== null || hosting.busy) return;
    void hosting.add(newAccount(draft));
  }
</script>

{#if hosting.adding}
  <TemplateDialog
    title="Add Hosting Account"
    onclose={() => hosting.closeAdd()}
    status={hosting.busy ? null : problem}
    actions={[{ label: hosting.busy ? "Checking…" : "Add", onclick: submit, primary: true, disabled: problem !== null || hosting.busy }]}
  >
    <div class="tabs" role="tablist" aria-label="Hosting provider">
      {#each PROVIDERS as entry (entry.id)}
        <button
          type="button"
          role="tab"
          aria-selected={provider === entry.id}
          class:active={provider === entry.id}
          disabled={hosting.busy}
          onclick={() => (provider = entry.id)}>{entry.label}</button
        >
      {/each}
    </div>
    {#if hosting.addError}
      <Callout kind="danger"><span class="error">{hosting.addError}</span></Callout>
    {/if}
    {#if selfHosted(provider)}
      <label class="field">
        <span class="lbl">Server URL</span>
        <input type="text" class="mono" spellcheck="false" placeholder={serverPlaceholder(provider)} bind:value={server} />
      </label>
    {:else}
      <label class="field">
        <span class="lbl">Atlassian account email</span>
        <input type="text" spellcheck="false" autocomplete="email" bind:value={email} />
      </label>
    {/if}
    <div class="field">
      <label class="lbl" for="hosting-token">Access token</label>
      <span class="secret">
        <input
          id="hosting-token"
          type={shown ? "text" : "password"}
          spellcheck="false"
          autocomplete="off"
          data-autofocus
          bind:value={token}
        />
        <button
          type="button"
          class="btn sm eye"
          aria-pressed={shown}
          aria-label={shown ? "Hide token" : "Show token"}
          title={shown ? "Hide token" : "Show token"}
          onclick={() => (shown = !shown)}
        >
          <svg viewBox="0 0 16 16" aria-hidden="true"
            ><path d="M1.5 8S4 3.5 8 3.5 14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8Z" /><circle cx="8" cy="8" r="2" />{#if shown}<path
                d="M2.5 13.5 13.5 2.5"
              />{/if}</svg
          >
        </button>
      </span>
      <p class="sub">{tokenHint(provider)}</p>
    </div>
    <OptionRow
      bind:checked={useForGit}
      label="Use the token for Git over HTTPS"
      hint="For this account's https:// remotes that have no sign-in of their own in Authentication"
    />
    <p class="sub">Add checks the token with the provider and keeps it in Cogit's encrypted password store.</p>
  </TemplateDialog>
{/if}

<style>
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

  .tabs {
    display: flex;
    align-self: flex-start;
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    overflow: hidden;
  }

  .tabs button {
    padding: var(--sp-2) var(--sp-5);
    background: var(--surface-input);
    border: 0;
    border-radius: 0;
    color: var(--text-secondary);
    font: inherit;
    cursor: default;
  }

  .tabs button:hover:not(:disabled) {
    background: var(--state-hover);
  }

  .tabs button:disabled {
    color: var(--fg-disabled);
  }

  .tabs button + button {
    border-left: 1px solid var(--field-border);
  }

  .tabs button.active {
    background: var(--state-selected);
    color: var(--text-primary);
  }

  .secret {
    display: flex;
    gap: var(--sp-2);
  }

  .secret .eye {
    align-self: stretch;
    height: auto;
    padding-block: 0;
  }

  .secret input {
    flex: 1 1 auto;
    min-width: 0;
  }

  .eye svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.3;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .error {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
