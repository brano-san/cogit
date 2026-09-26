<script lang="ts">
  import { onDestroy } from "svelte";
  import Checkbox from "$components/common/Checkbox.svelte";
  import { commitBox } from "$stores/commit-box.svelte";
  import { amends, canCommit, draftToSave, initialMessage, messageAfterCommit } from "$lib/commit-draft";
  import { SUBJECT_HARD, SUBJECT_SOFT, subjectOf, subjectState } from "$lib/commit-message";

  interface Props {
    /** What the button will actually commit, given the active filter (T6.8). */
    scope: import("$lib/commit-scope").CommitScope;
    stagedCount: number;
    busy?: boolean;
    draftKey: string;
    /** `commit.template` from the config; seeds an empty draft and the field after each
        commit, never overwrites a draft. */
    template?: string | null;
    /** HEAD has no commit yet: nothing to amend. */
    unborn?: boolean;
    /** `false` means nothing was committed (a question was cancelled, a hook refused). */
    oncommit: (message: string, amend: boolean, noVerify: boolean) => Promise<boolean> | void;
  }

  let { scope, stagedCount, busy = false, draftKey, template = null, unborn = false, oncommit }: Props =
    $props();

  let message = $state("");
  let amend = $state(false);
  let noVerify = $state(false);
  let committing = $state(false);

  const overflow = $derived(subjectState(message));
  const length = $derived([...subjectOf(message)].length);
  const amending = $derived(amends({ amend, unborn }));
  // Ticked in a repository that had commits, it would sit there disabled and ticked.
  $effect(() => {
    if (unborn) amend = false;
  });
  const ready = $derived(
    canCommit({ message, template, stagedCount, amend, busy, committing, scopeEmpty: scope.empty, unborn }),
  );

  // Cleared once the commit is made, not before: a cancelled question or a hook that
  // refused left the box empty, and a retry without Amend made a new commit instead.
  async function submit() {
    if (!ready) return;
    committing = true;
    let made: boolean | void;
    try {
      made = await oncommit(message, amending, noVerify);
    } finally {
      committing = false;
    }
    if (made === false) return;
    message = messageAfterCommit(template);
    amend = false;
    noVerify = false;
  }

  let field: HTMLTextAreaElement | undefined = $state();

  // Local ▸ Commit… (Ctrl+Enter), Commit with Amend (Ctrl+Shift+Enter) and Ctrl+K reach the
  // box from anywhere in the window (11 §4).
  onDestroy(
    commitBox.attach({
      focus: () => field?.focus(),
      submit: async (withAmend) => {
        if (withAmend) {
          // Commit with Amend before the first commit: there is nothing to amend.
          if (unborn) return;
          amend = true;
        }
        await submit();
      },
    }),
  );

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      void submit();
    }
  }

  // A draft survives a restart: losing a half-written message to a stray click is worse
  // than the cost of one key per repository in browser storage.
  $effect(() => {
    try {
      message = initialMessage(localStorage.getItem(draftKey), template);
    } catch {
      message = initialMessage(null, template);
    }
  });

  $effect(() => {
    try {
      const kept = draftToSave(message, template);
      if (kept === null) localStorage.removeItem(draftKey);
      else localStorage.setItem(draftKey, kept);
    } catch {
      // Private windows and blocked site data are not a reason to break committing.
    }
  });
</script>

<div class="box">
  <textarea
    bind:this={field}
    bind:value={message}
    {onkeydown}
    rows="3"
    placeholder="Commit message — Ctrl+Enter to commit"
    aria-label="Commit message"
  ></textarea>

  {#if scope.warning}
    <p class="hidden-warning">{scope.warning}</p>
  {/if}

  <div class="bar">
    <span
      class="count {overflow}"
      title="Length of the subject line (the first line): {length} characters. Keep it under {SUBJECT_SOFT}; past {SUBJECT_HARD} tools cut it off."
      >Subject <span class="tabular">{length}</span></span
    >
    <span class="option"
      ><Checkbox
        bind:checked={amend}
        label="Amend"
        disabled={unborn}
        title={unborn ? "Nothing to amend: this branch has no commits yet" : undefined}
      /></span
    >
    <span class="option"><Checkbox bind:checked={noVerify} label="No verify" /></span>
    <span class="grow"></span>
    <button type="button" disabled={!ready} onclick={submit}>
      {amending ? "Amend" : scope.label}
    </button>
  </div>
</div>

<style>
  /* The field takes what is left and gives it back first: the controls under it stay on
     screen however low the panel is dragged (R-183). */
  .box {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
    border-top: 1px solid var(--divider);
    padding: var(--sp-3) var(--sp-4);
  }

  textarea {
    display: block;
    flex: 1 1 auto;
    width: 100%;
    min-height: calc(2 * var(--lh-code) + 2 * var(--sp-3) + 2px);
    box-sizing: border-box;
    resize: none;
    padding: var(--sp-3);
    line-height: var(--lh-code);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-family: var(--font-mono);
    font-size: var(--fs-code);
  }

  .hidden-warning {
    flex: none;
    margin: var(--sp-3) 0 0;
    color: var(--status-modify);
    font-size: 10px;
  }

  .bar {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--sp-4);
    margin-top: var(--sp-3);
    font-size: var(--fs-dense);
  }

  .grow {
    flex: 1 1 auto;
  }

  .option {
    color: var(--text-secondary);
  }

  .count {
    font-size: 11px;
    color: var(--text-secondary);
    white-space: nowrap;
  }

  .count .tabular {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
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
