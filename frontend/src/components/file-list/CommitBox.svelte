<script lang="ts">
  import { SUBJECT_HARD, SUBJECT_SOFT, subjectOf, subjectState } from "$lib/commit-message";

  interface Props {
    stagedCount: number;
    busy?: boolean;
    draftKey: string;
    oncommit: (message: string, amend: boolean, noVerify: boolean) => void;
  }

  let { stagedCount, busy = false, draftKey, oncommit }: Props = $props();

  let message = $state("");
  let amend = $state(false);
  let noVerify = $state(false);

  const overflow = $derived(subjectState(message));
  const length = $derived([...subjectOf(message)].length);
  const ready = $derived(message.trim() !== "" && (stagedCount > 0 || amend) && !busy);

  function submit() {
    if (!ready) return;
    oncommit(message, amend, noVerify);
    message = "";
    amend = false;
    noVerify = false;
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      submit();
    }
  }

  // A draft survives a restart: losing a half-written message to a stray click is worse
  // than the cost of one key per repository in browser storage.
  $effect(() => {
    try {
      message = localStorage.getItem(draftKey) ?? "";
    } catch {
      message = "";
    }
  });

  $effect(() => {
    try {
      if (message === "") localStorage.removeItem(draftKey);
      else localStorage.setItem(draftKey, message);
    } catch {
      // Private windows and blocked site data are not a reason to break committing.
    }
  });
</script>

<div class="box">
  <textarea
    bind:value={message}
    {onkeydown}
    rows="3"
    placeholder="Commit message — Ctrl+Enter to commit"
    aria-label="Commit message"
  ></textarea>

  <div class="bar">
    <span class="count {overflow}" title="Subject line: {SUBJECT_SOFT} soft, {SUBJECT_HARD} hard"
      >{length}</span
    >
    <label><input type="checkbox" bind:checked={amend} /> Amend</label>
    <label><input type="checkbox" bind:checked={noVerify} /> No verify</label>
    <span class="grow"></span>
    <button type="button" disabled={!ready} onclick={submit}>
      {amend ? "Amend" : "Commit"}{stagedCount > 0 ? ` ${stagedCount}` : ""}
    </button>
  </div>
</div>

<style>
  .box {
    flex: 0 0 auto;
    border-top: 1px solid var(--divider);
    padding: var(--sp-3) var(--sp-4);
  }

  textarea {
    display: block;
    width: 100%;
    box-sizing: border-box;
    resize: vertical;
    padding: var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-family: var(--font-mono);
    font-size: var(--fs-code);
  }

  .bar {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    margin-top: var(--sp-3);
    font-size: var(--fs-dense);
  }

  .grow {
    flex: 1 1 auto;
  }

  label {
    display: flex;
    align-items: center;
    gap: var(--sp-2, 3px);
    color: var(--text-secondary);
  }

  .count {
    font-family: var(--font-mono);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    color: var(--text-secondary);
  }

  .count.long {
    color: var(--status-modify);
  }

  .count.too-long {
    color: var(--status-delete);
    font-weight: 600;
  }

  button {
    height: 22px;
    padding: 0 var(--sp-5);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  button:disabled {
    opacity: 0.45;
  }

  button:not(:disabled):hover {
    border-color: var(--status-ref);
  }
</style>
