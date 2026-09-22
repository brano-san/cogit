<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import { tokenizeConfigLine } from "$lib/config-syntax";
  import type { ConfigFile } from "$lib/ipc";
  import { untrack } from "svelte";

  /** Repository ▸ Edit Git Config. Built in rather than handed to an external editor:
      the text has to pass `git config --file` before it is written, and an editor that
      saves on its own would skip that check (doc/12-risks.md, R-155). */
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
  let top = $state(0);
  let left = $state(0);

  const lines = $derived(text.split("\n"));
  const changed = $derived(text !== file.text);
</script>

<Dialog {title} onclose={oncancel} width="min(860px, 94vw)" height="min(640px, 88vh)">
  <div class="config">
    <p class="path mono" title={file.path}>
      {file.path}{#if !file.exists}<span> — does not exist yet; Save creates it</span>{/if}
    </p>

    {#if problem}
      <div class="problem" role="alert">
        <strong>Git refused this config{problem.line ? ` at line ${problem.line}` : ""}.</strong>
        Nothing was saved.
        <pre class="mono">{problem.message}</pre>
      </div>
    {/if}

    <div class="editor">
      <div class="gutter mono" aria-hidden="true">
        <div style:transform="translateY({-top}px)">
          {#each lines as _, at (at)}
            <div class:bad={problem?.line === at + 1}>{at + 1}</div>
          {/each}
        </div>
      </div>
      <div class="stack">
        <pre class="paint mono" aria-hidden="true" style:transform="translate({-left}px, {-top}px)">{#each lines as line, at (at)}<div
              class="line"
              class:bad={problem?.line === at + 1}>{#each tokenizeConfigLine(line) as token, index (index)}<span
                class={token.cls}>{token.text}</span
              >{/each}</div>{/each}</pre>
        <textarea
          class="mono"
          spellcheck="false"
          wrap="off"
          aria-label="Git config"
          bind:value={text}
          onscroll={(event) => {
            top = event.currentTarget.scrollTop;
            left = event.currentTarget.scrollLeft;
          }}
        ></textarea>
      </div>
    </div>
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

  .path {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .problem {
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

  .editor {
    display: flex;
    flex: 1 1 auto;
    min-height: 0;
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    background: var(--surface-input);
    overflow: hidden;
  }

  .gutter {
    flex: 0 0 auto;
    min-width: 40px;
    padding: var(--sp-4) var(--sp-3);
    overflow: hidden;
    color: var(--text-secondary);
    text-align: right;
    border-right: 1px solid var(--divider);
    user-select: none;
  }

  .gutter .bad {
    color: var(--status-delete);
    font-weight: 700;
  }

  .stack {
    position: relative;
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
  }

  /* The two layers must lay text out identically, or the caret drifts off the colours. */
  .gutter,
  .paint,
  textarea {
    font-family: var(--font-mono);
    font-size: var(--fs-code);
    line-height: var(--lh-code);
    tab-size: 4;
  }

  .gutter > div > div,
  .line {
    height: var(--lh-code);
  }

  .paint {
    position: absolute;
    inset: 0 auto auto 0;
    margin: 0;
    padding: var(--sp-4);
    color: var(--text-code);
    white-space: pre;
    pointer-events: none;
  }

  .line.bad {
    background: color-mix(in srgb, var(--status-delete) 18%, transparent);
  }

  textarea {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    margin: 0;
    padding: var(--sp-4);
    background: transparent;
    color: transparent;
    caret-color: var(--text-primary);
    border: 0;
    outline: none;
    resize: none;
    white-space: pre;
    overflow: auto;
  }

  textarea::selection {
    background: color-mix(in srgb, var(--status-ref) 35%, transparent);
    color: transparent;
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
