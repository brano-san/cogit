<script lang="ts">
  import { tick } from "svelte";
  import CodeArea from "$components/common/CodeArea.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import EmptyState from "$components/common/EmptyState.svelte";
  import { hitSource, lineNotes, type LineNote } from "$lib/ignore-editor";
  import { tokenizeIgnoreLine } from "$lib/ignore-syntax";
  import {
    ignoreFiles,
    ignorePreview,
    saveIgnoreFile,
    type IgnoreFile,
    type IgnoreHit,
    type IgnorePreview,
    type IgnoreScope,
    type RepoId,
  } from "$lib/ipc";
  import { fitPath } from "$lib/truncate";
  import { errors } from "$stores/errors.svelte";

  interface Props {
    repo: RepoId;
    dir: string;
    onclose: () => void;
    onsaved: () => void;
  }

  let { repo, dir, onclose, onsaved }: Props = $props();

  const PREVIEW_PAUSE_MS = 300;
  const NOTES: Record<LineNote, string> = {
    duplicate: "The same pattern is on an earlier line",
    unused: "No file in the working tree matches this pattern",
  };

  let files = $state.raw<IgnoreFile[]>([]);
  let texts = $state<Record<IgnoreScope, string>>({ folder: "", exclude: "", global: "" });
  let active = $state<IgnoreScope>("folder");
  let preview = $state.raw<IgnorePreview | null>(null);
  let checking = $state(false);
  let saving = $state(false);
  let field: HTMLTextAreaElement | undefined = $state();
  let run = 0;

  const current = $derived(files.find((file) => file.scope === active));
  const changed = (file: IgnoreFile) => texts[file.scope] !== file.text;
  const dirty = $derived(files.some(changed));
  const notes = $derived(current ? lineNotes(texts[active], { scope: active, dir: current.dir }, preview) : new Map<number, LineNote>());
  const tracked = $derived(preview?.hits.filter((hit) => hit.tracked).length ?? 0);

  $effect(() => {
    void ignoreFiles(repo, dir)
      .then((loaded) => {
        files = loaded;
        for (const file of loaded) texts[file.scope] = file.text;
      })
      .catch((err: unknown) => {
        errors.report(err, "Could not read the ignore files");
        onclose();
      });
  });

  $effect(() => {
    if (files.length === 0) return;
    const drafts = files.map((file) => ({ scope: file.scope, dir: file.dir, text: texts[file.scope] }));
    const mine = ++run;
    checking = true;
    const timer = setTimeout(() => {
      void ignorePreview(repo, drafts)
        .then((next) => {
          if (mine === run) preview = next;
        })
        .catch((err: unknown) => errors.report(err, "Could not preview the ignore rules"))
        .finally(() => {
          if (mine === run) checking = false;
        });
    }, PREVIEW_PAUSE_MS);
    return () => clearTimeout(timer);
  });

  function tabLabel(file: IgnoreFile): string {
    if (file.scope === "exclude") return "Exclude (this clone)";
    if (file.scope === "global") return "Global";
    return file.dir === "" ? ".gitignore" : `${file.dir}/.gitignore`;
  }

  function opens(hit: IgnoreHit): boolean {
    return files.some((file) => file.scope === hit.scope && file.dir === hit.dir);
  }

  async function show(hit: IgnoreHit) {
    active = hit.scope;
    await tick();
    if (!field) return;
    const lines = texts[active].split("\n");
    const start = lines.slice(0, hit.line - 1).reduce((sum, line) => sum + line.length + 1, 0);
    field.focus();
    field.setSelectionRange(start, start + (lines[hit.line - 1]?.length ?? 0));
    const height = parseFloat(getComputedStyle(field).lineHeight) || 18;
    field.scrollTop = Math.max(0, (hit.line - 3) * height);
  }

  async function save() {
    saving = true;
    let wrote = false;
    try {
      for (const file of files.filter(changed)) {
        const text = texts[file.scope];
        await saveIgnoreFile(repo, { scope: file.scope, dir: file.dir, text });
        wrote = true;
        files = files.map((each) => (each === file ? { ...each, text, exists: true } : each));
      }
      onclose();
    } catch (err) {
      errors.report(err, "Could not save the ignore file");
    } finally {
      saving = false;
      if (wrote) onsaved();
    }
  }
</script>

<Dialog title="Edit Ignore Rules" {onclose} {dirty} width="min(1040px, 96vw)" height="min(680px, 90vh)">
  <div class="ignore">
    <div class="tabs" role="tablist" aria-label="Ignore file">
      {#each files as file (file.scope)}
        <button
          type="button"
          role="tab"
          aria-selected={active === file.scope}
          class:active={active === file.scope}
          title={file.path}
          onclick={() => (active = file.scope)}
          >{tabLabel(file)}{#if changed(file)}<span class="dot" aria-label="changed"> •</span>{/if}</button
        >
      {/each}
    </div>

    {#if current}
      <p class="where">
        <span class="path mono" title={current.path} use:fitPath={current.path}></span>
        {#if !current.exists}<span class="note">— does not exist yet; Save creates it</span>{/if}
      </p>
    {/if}

    <div class="main">
      {#key active}
        <CodeArea
          bind:text={texts[active]}
          bind:field
          label={current ? tabLabel(current) : "Ignore file"}
          tokenize={tokenizeIgnoreLine}
          markOf={(line) => {
            const note = notes.get(line);
            return note ? { cls: note, title: NOTES[note] } : null;
          }}
        />
      {/key}

      <aside class="preview" aria-label="Ignored files">
        <p class="head">
          {#if preview}
            Ignored: {preview.total}{#if tracked > 0}, {tracked} tracked{/if}{#if checking}<span class="muted"> · checking…</span>{/if}
          {:else}
            Checking…
          {/if}
        </p>
        {#if preview && preview.hits.length === 0}
          <EmptyState title="Nothing in the working tree is ignored." />
        {:else if preview}
          <ul>
            {#each preview.hits as hit (hit.path)}
              <li class:tracked={hit.tracked}>
                <span class="file truncate mono" title={hit.path}>{hit.path}</span>
                {#if hit.tracked}<span class="badge" title="Tracked: an ignore rule does not untrack a file">tracked</span>{/if}
                {#if opens(hit)}
                  <button type="button" class="source mono" title={hit.pattern} onclick={() => void show(hit)}
                    >{hitSource(hit)}</button
                  >
                {:else}
                  <span class="source mono" title={hit.pattern}>{hitSource(hit)}</span>
                {/if}
              </li>
            {/each}
          </ul>
          {#if preview.total > preview.hits.length}
            <p class="more">and {preview.total - preview.hits.length} more</p>
          {/if}
        {/if}
      </aside>
    </div>
  </div>

  {#snippet footer()}
    <span class="hint">The list follows the text as you type; nothing is written until Save.</span>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="button" class="btn primary" disabled={saving || !dirty} onclick={() => void save()}
      >{saving ? "Saving…" : "Save"}</button
    >
  {/snippet}
</Dialog>

<style>
  .ignore {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    height: 100%;
    min-height: 0;
  }

  .tabs {
    display: flex;
    flex: none;
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

  .tabs button:hover {
    background: var(--state-hover);
  }

  .tabs button + button {
    border-left: 1px solid var(--field-border);
  }

  .tabs button.active {
    background: var(--state-selected);
    color: var(--text-primary);
  }

  .dot {
    color: var(--status-modify);
  }

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

  .path {
    flex: 1 1 0;
    min-width: 0;
    overflow: hidden;
  }

  .note {
    flex: none;
  }

  .main {
    display: flex;
    flex: 1 1 0;
    gap: var(--sp-4);
    min-height: 0;
  }

  .main > :global(.editor) {
    flex: 3 1 0;
  }

  .preview {
    display: flex;
    flex: 2 1 0;
    flex-direction: column;
    min-width: 240px;
    min-height: 0;
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
  }

  .head {
    flex: none;
    margin: 0;
    padding: var(--sp-3) var(--sp-4);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-dense);
  }

  .muted {
    color: var(--text-secondary);
  }

  ul {
    flex: 1 1 auto;
    min-height: 0;
    margin: 0;
    padding: 0;
    overflow: auto;
    list-style: none;
  }

  li {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-height: var(--h-row);
    padding: 0 var(--sp-4);
    font-size: var(--fs-dense);
  }

  li:hover {
    background: var(--state-hover);
  }

  .file {
    flex: 1 1 auto;
    min-width: 0;
  }

  li.tracked .file {
    color: var(--status-modify);
  }

  .badge {
    flex: none;
    color: var(--status-modify);
    font-size: var(--fs-small);
  }

  .source {
    flex: none;
    padding: 0;
    color: var(--text-secondary);
    font-size: var(--fs-small);
  }

  button.source:hover {
    color: var(--link);
    text-decoration: underline;
  }

  .more {
    flex: none;
    margin: 0;
    padding: var(--sp-2) var(--sp-4);
    color: var(--text-secondary);
    font-size: var(--fs-small);
  }

  .hint {
    flex: 1 1 auto;
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }
</style>
