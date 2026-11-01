<script lang="ts">
  import CodeArea from "$components/common/CodeArea.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import { tokenizeConfigLine } from "$lib/config-syntax";
  import type { ConfigFile } from "$lib/ipc";
  import { fitPath } from "$lib/truncate";
  import { untrack } from "svelte";

  /** Repository ▸ Edit Git Config, both scopes. Built in rather than handed to an external
      editor: the text has to pass `git config --file` before it is written, and an editor
      that saves on its own would skip that check (doc/12-risks.md, R-155). */
  interface Props {
    title: string;
    file: ConfigFile;
    /** Git's refusal of the last Save, with the line it named. */
    problem: { line: number | null; message: string } | null;
    saving: boolean;
    onsave: (text: string) => void;
    oncancel: () => void;
  }

  let { title, file, problem, saving, onsave, oncancel }: Props = $props();

  let text = $state(untrack(() => file.text));
  const changed = $derived(text !== file.text);
</script>

<Dialog {title} onclose={oncancel} dirty={changed} width="min(860px, 94vw)" height="min(640px, 88vh)">
  <div class="config">
    <p class="where">
      <span class="path mono" title={file.path} use:fitPath={file.path}></span>
      {#if !file.exists}<span class="note">— does not exist yet; Save creates it</span>{/if}
    </p>

    {#if problem}
      <div class="problem" role="alert">
        <strong>Git refused this config{problem.line ? ` at line ${problem.line}` : ""}.</strong>
        Nothing was saved.
        <pre class="mono">{problem.message}</pre>
      </div>
    {/if}

    <CodeArea
      bind:text
      label="Git config"
      tokenize={tokenizeConfigLine}
      markOf={(line) => (problem?.line === line ? { cls: "bad", title: problem.message } : null)}
    />
  </div>

  {#snippet footer()}
    <span class="grow"></span>
    <button type="button" class="btn" onclick={oncancel}>Cancel</button>
    <button
      type="button"
      class="btn primary"
      disabled={saving || (!changed && file.exists)}
      onclick={() => onsave(text)}>{saving ? "Checking…" : "Save"}</button
    >
  {/snippet}
</Dialog>

<style>
  .config {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    height: 100%;
    min-height: 0;
  }

  /* Never squeezed (#22): the editor below takes what is left, whatever its line count. */
  .where {
    display: flex;
    align-items: baseline;
    gap: var(--sp-2);
    flex: none;
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
    white-space: nowrap;
  }

  /* Cut in the middle to its width by `fitPath`: the folder and the file name both stay. */
  .path {
    flex: 1 1 0;
    min-width: 0;
    overflow: hidden;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .note {
    flex: none;
  }

  .problem {
    flex: none;
    max-height: 40%;
    overflow: auto;
    padding: var(--sp-3) var(--sp-4);
    border-left: 3px solid var(--status-delete);
    background: var(--surface-raised);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
  }

  .problem pre {
    margin: var(--sp-2) 0 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
