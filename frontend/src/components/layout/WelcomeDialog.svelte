<script lang="ts">
  import { tick } from "svelte";
  import Checkbox from "$components/common/Checkbox.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import { isUnavailable, noteFor, okAction, type MruRow, type WelcomeAction, type WelcomeOption } from "$lib/welcome";
  import type { WelcomeDialog } from "$stores/welcome.svelte";

  /** Repository ▸ Welcome…, and the start of a session with nothing to restore (F-586). */
  interface Props {
    dialog: WelcomeDialog;
    rows: readonly MruRow[];
    showAtStart: boolean;
    onshowchange: (show: boolean) => void;
    onrun: (action: WelcomeAction) => void;
    onforget: (path: string) => void;
    oncontext: (path: string, x: number, y: number) => void;
    onclose: () => void;
  }

  let { dialog, rows, showAtStart, onshowchange, onrun, onforget, oncontext, onclose }: Props = $props();

  let content: HTMLDivElement | undefined = $state();
  const selection = $derived(dialog.selection);
  const action = $derived(okAction(selection, rows));
  const selectedRow = $derived(selection.option === 3 && selection.row !== null ? (rows[selection.row] ?? null) : null);
  const blocked = $derived(selectedRow !== null && isUnavailable(dialog.availability.get(selectedRow.path)));
  const canRun = $derived(action !== null && !blocked);

  const OPTIONS: { option: WelcomeOption; label: string; hint?: string }[] = [
    {
      option: 1,
      label: "Add an existing local or create a new repository",
      hint: "Choose a folder. If it is not a repository yet, you can initialize one there.",
    },
    { option: 2, label: "Clone existing repository", hint: "Download a repository from a URL." },
    { option: 3, label: "Reopen previously used repository" },
  ];

  /** The one element that holds the focus, as in a listbox: Tab leaves, the arrows walk. */
  function focusCurrent() {
    void tick().then(() => content?.querySelector<HTMLElement>('[data-stop="current"]')?.focus());
  }

  function run() {
    if (action && canRun) onrun(action);
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      dialog.move(event.key === "ArrowDown" ? "down" : "up", rows.length);
      focusCurrent();
    } else if (event.key === "Delete" && selectedRow) {
      event.preventDefault();
      forget(selectedRow.path);
    }
  }

  function forget(path: string) {
    onforget(path);
    focusCurrent();
  }

  function pickOption(option: WelcomeOption) {
    dialog.choose(option, rows.length);
    focusCurrent();
  }

  function pickRow(index: number) {
    dialog.chooseRow(index);
    focusCurrent();
  }

  const stopOption = (option: WelcomeOption) =>
    selection.option === option && !(option === 3 && rows.length > 0);
</script>

<Dialog
  title="Welcome to Cogit"
  {onclose}
  onconfirm={run}
  width="min(680px, 94vw)"
  surface="var(--bg-elevated)"
>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="welcome" bind:this={content} {onkeydown}>
    <h3 id="welcome-question">What do you want to do?</h3>
    <p class="lead">
      Open or create a local repository, clone a remote one, or open a previously used one.
    </p>

    <div class="options" role="radiogroup" aria-labelledby="welcome-question">
      {#each OPTIONS as entry (entry.option)}
        {@const checked = selection.option === entry.option}
        <div class="option">
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div
            class="choice"
            class:checked
            role="radio"
            aria-checked={checked}
            tabindex={stopOption(entry.option) ? 0 : -1}
            data-stop={stopOption(entry.option) ? "current" : undefined}
            data-autofocus={stopOption(entry.option) ? "" : undefined}
            onclick={() => pickOption(entry.option)}
            ondblclick={() => {
              if (entry.option !== 3) onrun({ kind: entry.option === 1 ? "folder" : "clone" });
            }}
          >
            <span class="dot" aria-hidden="true"></span>
            <span class="text">
              <span class="label">{entry.label}</span>
              {#if entry.hint}<span class="hint">{entry.hint}</span>{/if}
            </span>
          </div>

          {#if entry.option === 3}
            {#if rows.length > 0}
              <div class="list" role="listbox" aria-label="Previously used repositories">
                {#each rows as row, index (row.path)}
                  {@const state = dialog.availability.get(row.path)}
                  {@const note = noteFor(state ?? "checking")}
                  {@const current = selection.option === 3 && selection.row === index}
                  <!-- svelte-ignore a11y_click_events_have_key_events -->
                  <div
                    class="row"
                    class:selected={current}
                    class:gone={isUnavailable(state)}
                    role="option"
                    aria-selected={current}
                    aria-label={note ? `${row.name}, ${row.path}, ${note}` : `${row.name}, ${row.path}`}
                    tabindex={current ? 0 : -1}
                    data-stop={current ? "current" : undefined}
                    data-autofocus={current ? "" : undefined}
                    onclick={() => pickRow(index)}
                    ondblclick={() => {
                      if (!isUnavailable(state)) onrun({ kind: "open", path: row.path });
                    }}
                    oncontextmenu={(event) => {
                      event.preventDefault();
                      pickRow(index);
                      oncontext(row.path, event.clientX, event.clientY);
                    }}
                  >
                    <span class="name">{row.name}{#if note}<span class="note">({note})</span>{/if}</span>
                    <span class="path">{row.path}</span>
                  </div>
                {/each}
              </div>
            {:else}
              <p class="empty">No repository has been opened yet.</p>
            {/if}
          {/if}
        </div>
      {/each}
    </div>
  </div>

  {#snippet footer()}
    <Checkbox
      wide
      checked={showAtStart}
      onchange={onshowchange}
      label="Show this dialog if no repository was opened"
    />
    <button type="button" class="btn" onclick={onclose}>Close</button>
    <button
      type="button"
      class="btn primary"
      disabled={!canRun}
      title={blocked ? "This repository is not available" : undefined}
      onclick={run}>OK</button
    >
  {/snippet}
</Dialog>

<style>
  .welcome {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    font-size: var(--fs-dense);
  }

  h3 {
    margin: 0;
    font-size: var(--fs-header);
    font-weight: 600;
    color: var(--fg-primary);
  }

  .lead {
    margin: 0;
    color: var(--fg-muted);
    line-height: 1.45;
  }

  .options {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    margin-top: var(--sp-3);
  }

  .option {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }

  .choice {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-4);
    padding: var(--sp-3) var(--sp-4);
    border-radius: var(--r-sm);
    color: var(--fg-primary);
    cursor: default;
  }

  .choice:hover {
    background: var(--bg-hover);
  }

  .choice:focus-visible,
  .row:focus-visible {
    outline: 1px solid var(--accent);
    outline-offset: -1px;
  }

  .dot {
    flex: 0 0 var(--checkbox-size);
    width: var(--checkbox-size);
    height: var(--checkbox-size);
    margin-top: max(calc((1lh - var(--checkbox-size)) / 2), 0px);
    background: var(--bg-input);
    border: 1px solid var(--border-strong);
    border-radius: 50%;
  }

  .choice.checked .dot {
    border-color: var(--accent);
    background: radial-gradient(circle, var(--accent) 0 3px, var(--bg-input) 3.5px);
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    min-width: 0;
  }

  .label {
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .hint {
    color: var(--fg-muted);
    line-height: 1.4;
  }

  .list {
    display: flex;
    flex-direction: column;
    max-height: 330px;
    margin-left: calc(var(--sp-4) + var(--checkbox-size) + var(--sp-4));
    overflow-y: auto;
    background: var(--bg-input);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-sm);
  }

  .row {
    display: flex;
    flex-direction: column;
    flex: 0 0 auto;
    gap: 1px;
    min-height: 40px;
    padding: var(--sp-3) var(--sp-4);
    box-sizing: border-box;
    cursor: default;
  }

  .row:hover {
    background: var(--bg-hover);
  }

  .row.selected {
    background: var(--bg-selected);
    box-shadow: inset 2px 0 0 var(--selected-bar);
  }

  .name {
    color: var(--fg-primary);
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .path {
    color: var(--fg-muted);
    overflow-wrap: anywhere;
  }

  .row.gone .name,
  .row.gone .path,
  .row.gone .note {
    color: var(--fg-disabled);
  }

  .note {
    margin-left: var(--sp-3);
    font-weight: 400;
  }

  .empty {
    margin: 0 0 0 calc(var(--sp-4) + var(--checkbox-size) + var(--sp-4));
    color: var(--fg-muted);
  }
</style>
