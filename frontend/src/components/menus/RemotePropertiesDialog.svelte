<script lang="ts">
  import Checkbox from "$components/common/Checkbox.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import type { RemoteInfo } from "$lib/ipc/remotes";

  /** Remote ▸ Properties, after SmartGit's: the URL and whether the background check asks
      this remote's server (R-554). */
  interface Props {
    info: RemoteInfo;
    /** Preferences ▸ how often the check runs; 0 when it is off for every remote. */
    everyMinutes: number;
    onsave: (url: string, backgroundFetch: boolean) => void;
    onclose: () => void;
  }

  let { info, everyMinutes, onsave, onclose }: Props = $props();

  // svelte-ignore state_referenced_locally
  let url = $state(info.url ?? "");
  // svelte-ignore state_referenced_locally
  let background = $state(info.backgroundFetch);

  const problem = $derived(
    url.trim() === "" ? "Enter the URL or path of the remote." : url.trim().startsWith("-") ? "Git will refuse that URL." : null,
  );
  const changed = $derived(url.trim() !== (info.url ?? "") || background !== info.backgroundFetch);

  function submit() {
    if (problem === null) onsave(url.trim(), background);
  }
</script>

<Dialog title="Properties of {info.name}" {onclose} onconfirm={submit} dirty={changed} width="min(560px, 92vw)">
  <div class="form">
    <label class="field">
      <span class="caption">URL or path</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input type="text" class="mono" bind:value={url} autofocus spellcheck="false" aria-label="URL or path" />
    </label>
    {#if info.pushUrl}
      <p class="hint">Pushes go to <span class="mono">{info.pushUrl}</span> (<span class="mono">pushurl</span>).</p>
    {/if}
    <div class="check">
      <Checkbox bind:checked={background} label="Perform background poll or fetch" />
      <p class="hint">
        {#if everyMinutes > 0}
          Every {everyMinutes} min Repositories asks this server whether there is anything to pull.
        {:else}
          The background check is off in Preferences for every remote.
        {/if}
      </p>
    </div>
  </div>

  {#snippet footer()}
    {#if problem}<span class="problem">{problem}</span>{/if}
    <span class="grow"></span>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="button" class="btn primary" disabled={problem !== null} onclick={submit}>Save</button>
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

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .check {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .caption {
    color: var(--text-secondary);
  }

  .hint {
    margin: 0;
    color: var(--text-secondary);
    overflow-wrap: anywhere;
  }

  .problem {
    color: var(--status-delete);
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
