<script lang="ts">
  import Checkbox from "$components/common/Checkbox.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import Radio from "$components/common/Radio.svelte";
  import Select from "$components/common/Select.svelte";
  import {
    customRefProblem,
    initialRemote,
    pushRefspec,
    pushTitle,
    targetRef,
    tracksByDefault,
    type PushSource,
    type PushTarget,
  } from "$lib/push-to";

  /** #28, after SmartGit's Push To — without its labels cut off on the right. */
  interface Props {
    source: PushSource;
    remotes: readonly string[];
    primary: string | null;
    /** `track`: the branch tracks what it becomes there (`--set-upstream`, R-550). */
    onpush: (remote: string, refspec: string, track: boolean) => void;
    onclose: () => void;
  }

  let { source, remotes, primary, onpush, onclose }: Props = $props();

  // svelte-ignore state_referenced_locally
  let remote = $state(initialRemote(source, remotes, primary) ?? "");
  let mode = $state<"tracked" | "custom">("tracked");
  let custom = $state("");
  // svelte-ignore state_referenced_locally
  let track = $state(tracksByDefault(source));
  let field: HTMLInputElement | undefined = $state();

  const target = $derived<PushTarget>(mode === "tracked" ? { mode } : { mode, ref: custom });
  const problem = $derived(
    remote === "" ? "This repository has no remote." : mode === "custom" ? customRefProblem(custom) : null,
  );
  const destination = $derived(remote === "" ? "" : targetRef(source, target, remote, remotes));

  function submit() {
    if (problem !== null) return;
    onpush(remote, pushRefspec(source, target, remote, remotes), source.kind === "branch" && track);
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
      <Radio name="push-to" checked={mode === "tracked"} onchange={() => (mode = "tracked")} label="Tracked or matching branch" />
      <Radio name="push-to" checked={mode === "custom"} onchange={() => (mode = "custom")} label="Custom Ref" />
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

    {#if source.kind === "branch"}
      <div class="field">
        <Checkbox bind:checked={track} label="Set upstream" />
        <span class="hint">{source.name} tracks the pushed branch from then on.</span>
      </div>
    {/if}
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
