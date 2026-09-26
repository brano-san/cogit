<script lang="ts">
  import Checkbox from "$components/common/Checkbox.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import Radio from "$components/common/Radio.svelte";
  import { shortOid } from "$lib/format";
  import { splitProblem, splitStarted, splitSummary } from "$lib/split-off";

  interface Props {
    oid: string;
    changed: readonly string[];
    published: boolean;
    busy: boolean;
    onsplit: (paths: string[], message: string, splitFirst: boolean) => void;
    onclose: () => void;
  }

  let { oid, changed, published, busy, onsplit, onclose }: Props = $props();

  let chosen = $state<string[]>([]);
  let message = $state("");
  let splitFirst = $state(true);

  const problem = $derived(splitProblem(changed, chosen, message));
  const summary = $derived(splitSummary(changed, chosen, splitFirst));
  const runnable = $derived(problem === null && !busy);

  function toggle(path: string) {
    chosen = chosen.includes(path) ? chosen.filter((p) => p !== path) : [...chosen, path];
  }

  function split() {
    if (runnable) onsplit(chosen, message.trim(), splitFirst);
  }
</script>

<Dialog
  title="Split off files from {shortOid(oid)}"
  {onclose}
  onconfirm={split}
  dirty={splitStarted(chosen, message)}
  width="min(560px, 90vw)"
  flush
>
  {#if published}
    <p class="danger">
      This commit is already on a remote. Splitting it rewrites every commit from here on, so
      the branch will need a force-push and anyone who pulled it will have to reset.
    </p>
  {/if}

  <div class="files">
    {#each changed as path (path)}
      <div class="row">
        <Checkbox wide checked={chosen.includes(path)} onchange={() => toggle(path)}>
          <span class="truncate">{path}</span>
        </Checkbox>
      </div>
    {/each}
  </div>

  <div class="form">
    <label class="field">
      <span>New commit message</span>
      <input type="text" bind:value={message} placeholder="What the split-off files do" />
    </label>

    <div class="field">
      <span>Position</span>
      <div class="choice">
        <Radio name="split-position" checked={splitFirst} onchange={() => (splitFirst = true)} label="Before the original" />
        <Radio name="split-position" checked={!splitFirst} onchange={() => (splitFirst = false)} label="After the original" />
      </div>
    </div>

    <p class="summary">{summary}</p>
  </div>

  {#snippet footer()}
    <span class="problem">{problem ?? ""}</span>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="button" class="btn primary" disabled={!runnable} onclick={split}>
      {busy ? "Splitting…" : "Split Off"}
    </button>
  {/snippet}
</Dialog>

<style>
  .danger {
    margin: 0;
    padding: var(--sp-4) var(--dialog-inset);
    background: var(--c-deleted-bg);
    color: var(--text-primary);
    font-size: var(--fs-dense);
    border-bottom: 1px solid var(--divider);
  }

  .files {
    flex: 1 1 auto;
    min-height: 0;
    padding: var(--sp-3) 0;
    overflow: auto;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    height: var(--h-row-dense);
    padding: 0 var(--dialog-inset);
    font-family: var(--font-mono);
    font-size: var(--fs-dense);
  }

  .row:hover {
    background: var(--state-hover);
  }

  .form {
    flex: 0 0 auto;
    padding: var(--sp-4) var(--dialog-inset);
    border-top: 1px solid var(--divider);
  }

  .field {
    display: flex;
    align-items: center;
    gap: var(--sp-5);
    min-height: var(--h-row);
    font-size: var(--fs-dense);
  }

  .field > span:first-child {
    flex: 0 0 150px;
    color: var(--text-secondary);
  }

  /* The shared dialog sizes text fields for a whole row; here the caption shares it. */
  .form .field input[type="text"] {
    flex: 1 1 auto;
    width: auto;
    min-width: 0;
  }

  .choice {
    display: flex;
    gap: var(--sp-6);
  }

  .summary {
    margin: var(--sp-3) 0 0;
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  .problem {
    flex: 1 1 auto;
    color: var(--status-modify);
    font-size: var(--fs-header);
  }
</style>
