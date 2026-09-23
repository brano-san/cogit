<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import Select from "$components/common/Select.svelte";
  import {
    customRefProblem,
    initialRemote,
    pushRefspec,
    pushTitle,
    targetRef,
    type PushSource,
    type PushTarget,
  } from "$lib/push-to";

  /** #28, after SmartGit's Push To — without its labels cut off on the right. */
  interface Props {
    source: PushSource;
    remotes: readonly string[];
    primary: string | null;
    onpush: (remote: string, refspec: string) => void;
    onclose: () => void;
  }

  let { source, remotes, primary, onpush, onclose }: Props = $props();

  // svelte-ignore state_referenced_locally
  let remote = $state(initialRemote(source, remotes, primary) ?? "");
  let mode = $state<"tracked" | "custom">("tracked");
  let custom = $state("");
  let field: HTMLInputElement | undefined = $state();

  const target = $derived<PushTarget>(mode === "tracked" ? { mode } : { mode, ref: custom });
  const problem = $derived(
    remote === "" ? "This repository has no remote." : mode === "custom" ? customRefProblem(custom) : null,
  );
  const destination = $derived(remote === "" ? "" : targetRef(source, target, remote, remotes));

  function submit() {
    if (problem !== null) return;
    onpush(remote, pushRefspec(source, target, remote, remotes));
  }

  $effect(() => {
    if (mode === "custom") field?.focus();
  });
</script>

<Dialog title="Push To" {onclose} onconfirm={submit} width="min(560px, 92vw)">
  <div class="form">
    <div class="heading">
      <h3>{pushTitle(source, remote || "—")}</h3>
      <p class="hint">Select the target repository where to push the ref(s).</p>
    </div>

    <div class="field">
      <span class="caption">Remote</span>
      {#if remotes.length > 1}
        <Select
          value={remote}
          label="Remote"
          options={remotes.map((name) => [name, name] as const)}
          onchange={(next) => (remote = next)}
        />
      {:else}
        <span class="mono">{remote || "none"}</span>
      {/if}
    </div>

    <fieldset>
      <legend class="caption">Push To:</legend>
      <label class="choice">
        <input type="radio" name="push-to" checked={mode === "tracked"} onchange={() => (mode = "tracked")} />
        <span>Tracked or matching branch</span>
      </label>
      <label class="choice">
        <input type="radio" name="push-to" checked={mode === "custom"} onchange={() => (mode = "custom")} />
        <span>Custom Ref</span>
      </label>
      <div class="nested">
        <input
          bind:this={field}
          type="text"
          class="mono"
          bind:value={custom}
          disabled={mode !== "custom"}
          placeholder={source.kind === "tag" ? "refs/tags/…" : "refs/heads/…"}
          aria-label="Custom ref"
        />
      </div>
      {#if destination}
        <p class="hint">Writes <span class="mono">{destination}</span> on {remote}.</p>
      {/if}
    </fieldset>
  </div>

  {#snippet footer()}
    {#if problem && (mode === "custom" ? custom !== "" : true)}<span class="problem">{problem}</span>{/if}
    <span class="grow"></span>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="button" class="btn primary" disabled={problem !== null} onclick={submit}>Push</button>
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

  .heading {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  h3 {
    margin: 0;
    font-size: var(--fs-ui);
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .field {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-4);
  }

  .caption {
    color: var(--text-secondary);
  }

  fieldset {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    margin: 0;
    padding: 0;
    border: 0;
    min-width: 0;
  }

  legend {
    padding: 0;
    margin-bottom: var(--sp-2);
  }

  .choice {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }

  .nested {
    padding-left: var(--sp-7);
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
