<script lang="ts">
  import { prettyKeys } from "$lib/keymap";
  import { onDestroy, tick, untrack } from "svelte";
  import Checkbox from "$components/common/Checkbox.svelte";
  import Button from "$components/common/Button.svelte";
  import Icon from "$components/common/Icon.svelte";
  import { aiSettings } from "$stores/ai-settings.svelte";
  import { AiMessageStore, generateMessageBlocker } from "$stores/ai-message.svelte";
  import { confirmation } from "$stores/confirm.svelte";
  import { notices } from "$stores/notices.svelte";
  import { repository } from "$stores/repository.svelte";
  import { settings } from "$stores/settings.svelte";
  import { cancelOperation, generateCommitMessage } from "$lib/ipc";
  import { commitBox } from "$stores/commit-box.svelte";
  import { afterCommit, amends, commitBlocker, draftToSave, initialMessage, messageAfterCommit } from "$lib/commit-draft";
  import { messageSeed } from "$lib/commit-prefix";
  import { SUBJECT_HARD, SUBJECT_SOFT, messageGuides, subjectLength, subjectState } from "$lib/commit-message";
  import { foreign, ON_MAC, primary } from "$lib/platform";
  import { spellField } from "$lib/spell-field.svelte";

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
    /** From the branch name (F-943): seeds an empty field when `template` does not. */
    prefix?: string | null;
    /** `false` means nothing was committed (a question was cancelled, a hook refused). */
    oncommit: (message: string, amend: boolean, noVerify: boolean) => Promise<boolean> | void;
  }

  let { scope, stagedCount, busy = false, draftKey, template = null, unborn = false, prefix = null, oncommit }: Props =
    $props();

  /** What an empty field opens with; git's own prefill is never mixed with the prefix. */
  const seed = $derived(messageSeed(template, prefix));

  let message = $state("");
  let amend = $state(false);
  let noVerify = $state(false);
  let committing = $state(false);

  const ai = new AiMessageStore({
    consent: (name, key) => confirmation.askWithOption({
      title: "Send Staged Changes",
      message: `The staged changes of ${name} will be sent to ${key.host}. Continue?`,
      fields: [["Service", key.service]],
      confirm: "Continue",
      warning: true,
      option: "Don't ask again for this service",
    }),
    replace: () => confirmation.ask({
      title: "Replace Commit Message",
      message: "Replace the current commit message with a generated suggestion?",
      confirm: "Replace",
    }),
    allowed: (service) => aiSettings.allowed(service),
    remember: (service) => aiSettings.remember(service),
    cancel: cancelOperation,
    generate: generateCommitMessage,
    report: (err) => notices.report(err, "Could not generate a commit message"),
  });

  const config = $derived(settings.current.ai);
  const key = $derived(aiSettings.at(config));
  const unavailable = $derived(
    busy || committing
      ? "Wait for the staged changes to load"
      : generateMessageBlocker(stagedCount, config, key, ai.active),
  );
  const generationScope = $derived(JSON.stringify([draftKey, repository.current?.head, prefix, config]));
  $effect(() => {
    void generationScope;
    return () => untrack(() => {
      ai.cancel();
      ai.shortened = false;
    });
  });

  async function generate(): Promise<void> {
    const repo = repository.current;
    if (!repo || !config || !key || unavailable) return;
    const held = generationScope;
    const owner = draftKey;
    await ai.start({
      config, key, prefix, repo: repo.repo, name: repo.name,
      read: () => message,
      write: (text) => { message = text; },
      valid: () => generationScope === held && stagedCount > 0,
      ownsField: () => draftKey === owner,
    });
  }

  const guides = $derived(messageGuides(message, settings.current.messageGuides, settings.current.messageSubjectColumn, settings.current.messageBodyColumn));
  const overflow = $derived(guides ? (guides.subjectOver ? "long" : "ok") : subjectState(message));
  const length = $derived(subjectLength(message));
  const amending = $derived(amends({ amend, unborn }));
  // Ticked in a repository that had commits, it would sit there disabled and ticked.
  $effect(() => {
    if (unborn) amend = false;
  });
  const blocked = $derived(
    commitBlocker({ message, template: seed, stagedCount, amend, busy, committing, scopeEmpty: scope.empty, unborn }),
  );
  const ready = $derived(blocked === null && !ai.active);

  // Cleared once the commit is made, not before: a cancelled question or a hook that
  // refused left the box empty, and a retry without Amend made a new commit instead.
  async function submit() {
    if (!ready) return;
    committing = true;
    const keyAtStart = draftKey;
    let made: boolean | void;
    try {
      made = await oncommit(message, amending, noVerify);
    } finally {
      committing = false;
    }
    const after = afterCommit(made !== false, keyAtStart, draftKey);
    if (after.forgetKey !== null) {
      try {
        localStorage.removeItem(after.forgetKey);
      } catch {
        // Blocked site data: the stale draft stays, nothing else breaks.
      }
    }
    if (!after.resetField) return;
    message = messageAfterCommit(seed);
    ai.shortened = false;
    void caretAfterPrefix();
    amend = false;
    noVerify = false;
  }

  let field: HTMLTextAreaElement | undefined = $state();

  /** A prefix is followed by what the user types: the caret goes after it. */
  async function caretAfterPrefix() {
    if (prefix === null || message !== prefix) return;
    await tick();
    if (field && message === prefix) field.setSelectionRange(message.length, message.length);
  }

  // Local ▸ Commit… (Ctrl+Enter), Commit with Amend (Ctrl+Shift+Enter) and Go to the Commit Message reach the
  // box from anywhere in the window (11 §4).
  onDestroy(
    commitBox.attach({
      focus: () => field?.focus(),
      generate,
      generating: () => ai.active,
      draft: () => ({ key: draftKey, text: message, write: (text) => { ai.cancel(); message = text; } }),
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
    if (event.key === "Enter" && primary(event, ON_MAC) && !foreign(event, ON_MAC) && !event.shiftKey && !event.altKey) {
      event.preventDefault();
      void submit();
    }
  }

  // A draft survives a restart: losing a half-written message to a stray click is worse
  // than the cost of one key per repository in browser storage.
  $effect(() => {
    try {
      message = initialMessage(localStorage.getItem(draftKey), seed);
    } catch {
      message = initialMessage(null, seed);
    }
    // Read outside: typing must not run this again.
    untrack(() => void caretAfterPrefix());
  });

  // The Commit window (Ctrl+K) keeps the same draft: what it writes or clears arrives here.
  $effect(() => {
    const key = draftKey;
    const onstorage = (event: StorageEvent) => {
      if (event.key !== key) return;
      const next = initialMessage(event.newValue, seed);
      if (next !== message) message = next;
    };
    window.addEventListener("storage", onstorage);
    return () => window.removeEventListener("storage", onstorage);
  });

  $effect(() => {
    if (ai.streaming) return;
    try {
      const kept = draftToSave(message, seed);
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
    placeholder="Commit message — {prettyKeys('CmdOrCtrl+Enter')} to commit"
    aria-label="Commit message"
    aria-busy={ai.streaming}
    readonly={ai.streaming}
    spellcheck="false"
    class:commit-message-guides={guides !== null}
    style:background-position={guides?.position}
    {@attach spellField(() => message)}
  ></textarea>

  {#if scope.warning}
    <p class="hidden-warning">{scope.warning}</p>
  {/if}
  {#if ai.shortened}
    <p class="hidden-warning" role="status">The staged diff was shortened for this suggestion.</p>
  {/if}

  <div class="bar">
    <span
      class="count {overflow}"
      title={guides
         ? `Subject length: ${length} characters. Guide: ${settings.current.messageSubjectColumn}.`
         : `Length of the subject line (the first line): ${length} characters. Keep it under ${SUBJECT_SOFT}; past ${SUBJECT_HARD} tools cut it off.`}
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
    {#if ai.streaming}
      <Button onclick={() => ai.cancel()} data-tip="Cancel message generation">Cancel</Button>
    {:else}
      {#if key?.state === "locked" && config}
        <Button onclick={() => { if (config) void aiSettings.unlock(config); }} data-tip="Unlock the password store">Unlock…</Button>
      {/if}
      <span class="generate" data-tip={unavailable ?? "Generate Message"}>
        <Button disabled={unavailable !== undefined} aria-label="Generate Message" onclick={generate}>
          <Icon name="generateMessage" />
        </Button>
      </span>
    {/if}
    <button
      type="button"
      disabled={!ready}
      title={blocked ?? undefined}
      onclick={submit}
    >
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
    background-color: var(--surface-input);
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
    font-size: var(--fs-small);
  }

  /* One row while it fits; narrower, the options wrap as whole units under the counter
     and the Commit button keeps its size at the right. A label never breaks. */
  .bar {
    display: flex;
    flex: none;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-4);
    margin-top: var(--sp-3);
    font-size: var(--fs-dense);
  }

  .grow {
    flex: 1 1 auto;
  }

  .generate {
    display: inline-flex;
  }

  .generate :global(.btn) {
    width: var(--h-button-sm);
    padding: 0;
  }

  .option {
    flex: none;
    white-space: nowrap;
    color: var(--text-secondary);
  }

  .count {
    flex: none;
    font-size: var(--fs-header);
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
    flex: none;
    margin-left: auto;
    height: 22px;
    padding: 0 var(--sp-5);
    background-color: var(--surface-input);
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
