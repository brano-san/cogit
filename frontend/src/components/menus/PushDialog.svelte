<script lang="ts">
  import { onMount, untrack } from "svelte";
  import PushList from "./PushList.svelte";
  import CopyButton from "$components/common/CopyButton.svelte";
  import Disclosure from "$components/common/Disclosure.svelte";
  import RevisionCombobox from "$components/common/RevisionCombobox.svelte";
  import Select from "$components/common/Select.svelte";
  import Callout from "$components/common/template/Callout.svelte";
  import OptionRow from "$components/common/template/OptionRow.svelte";
  import TemplateDialog from "$components/common/template/TemplateDialog.svelte";
  import { shortOid } from "$lib/format";
  import { toCogitError } from "$lib/ipc";
  import { remoteInfo } from "$lib/ipc/remotes";
  import {
    pushAtomicSupport,
    pushPreview,
    pushTagsPreview,
    type PushOptions,
    type PushPreview,
    type TagsMode,
  } from "$lib/ipc/network-dialogs";
  import {
    chooseLocal,
    pushChoiceOf,
    pushOptionsOf,
    pushProblem,
    remoteBranchChoices,
    type PushChoice,
  } from "$lib/network-dialogs";
  import {
    atomicGate,
    commitsText,
    effectiveChoice,
    leaseLine,
    protectedQuestion,
    pushButton,
    pushCommand,
    statusLine,
    notesNote,
    tagCounter,
    upstreamState,
    type TagCount,
  } from "$lib/push-dialog";
  import { splitUpstream } from "$lib/push-to";
  import { confirmation } from "$stores/confirm.svelte";
  import { networkDialog, type PushRequest } from "$stores/network-dialog.svelte";
  import { settings } from "$stores/settings.svelte";

  /** Push (item 12), the Push dialog: where, what goes, which tags and notes. */
  interface Props {
    request: PushRequest;
    /** `options` is what goes; `choice` is what Remember keeps. */
    onpush: (choice: PushChoice, options: PushOptions, remember: boolean) => void;
    /** Fetches `remote` and refreshes the panels; the dialog then reads everything again.
        `false`: the fetch did not run to the end. */
    onfetch: (remote: string) => Promise<boolean>;
    onclose: () => void;
  }

  let { request, onpush, onfetch, onclose }: Props = $props();

  /** The list never holds more; the count above it is exact. */
  const LIST_CAP = 200;
  /** Room for a remote's URL beside its name. */
  const REMOTE_LIST_WIDTH = 400;
  const TAG_CHOICES: readonly (readonly [TagsMode, string])[] = [
    ["none", "No tags"],
    ["follow", "Tags pointing to pushed commits (--follow-tags)"],
    ["all", "All tags (--tags)"],
  ];

  // svelte-ignore state_referenced_locally
  const opened = request.locals.find((b) => b.name === request.local);
  // svelte-ignore state_referenced_locally
  let choice = $state<PushChoice>(
    pushChoiceOf(request.defaults, {
      remote: request.remote,
      local: request.local,
      branch: request.branch,
      hasUpstream: (opened?.upstream ?? null) !== null,
    }),
  );
  let remember = $state(false);
  let preview = $state<PushPreview | null>(null);
  let listOpen = $state(false);
  let asked = 0;
  let fetching = $state(false);
  /** Grows after a Fetch: what was read from the remote-tracking refs is read again. */
  let revision = $state(0);

  /** When the dialog's Fetch of each remote ended: an empty remote leaves git nothing to date. */
  let fetchedHere = $state<Record<string, number>>({});

  async function fetchRemote() {
    const remote = choice.remote;
    fetching = true;
    try {
      if (await onfetch(remote)) fetchedHere[remote] = Date.now();
      revision += 1;
    } finally {
      fetching = false;
    }
  }

  const upstream = $derived(request.locals.find((b) => b.name === choice.local)?.upstream ?? null);
  const branches = $derived(remoteBranchChoices(choice.remote, request.remoteBranches, choice.local));
  const problem = $derived(pushProblem(choice));
  const tracking = $derived(upstreamState(upstream, choice.remote, choice.branch));
  const danger = $derived(choice.forceWithLease && !choice.dryRun);
  const lease = $derived(leaseLine(choice.remote, choice.branch, preview?.remoteTip ?? null));

  async function push() {
    if (danger && settings.current.protectedBranches.includes(choice.branch)) {
      if (!(await confirmation.ask(protectedQuestion(choice.remote, choice.branch)))) return;
    }
    onpush(choice, pushOptionsOf(sent, preview?.remoteTip ?? null), remember);
  }

  function chooseRemote(next: string) {
    choice.remote = next;
    // The branch of the same name on the new remote, as a first push would send it.
    const same = splitUpstream(upstream ?? "", request.remotes);
    choice.branch = same && same.remote === next ? same.branch : choice.local;
  }

  function pickLocal(name: string) {
    const branch = request.locals.find((b) => b.name === name);
    if (branch) choice = chooseLocal(choice, branch, request.remotes, request.defaults);
  }

  $effect(() => {
    const { local, remote, branch } = choice;
    void revision;
    if (problem !== null) return;
    const ticket = ++asked;
    pushPreview(request.repo, local, remote, branch, LIST_CAP)
      .then((found) => {
        if (ticket === asked) preview = found;
      })
      .catch(() => {
        if (ticket === asked) preview = null;
      });
  });

  /** After the last change, so a typed branch name is not asked of the remote letter by letter. */
  const TAG_DELAY_MS = 400;
  let tags = $state<TagCount | null>(null);
  let counted = 0;

  function failureLine(err: unknown): string {
    const error = toCogitError(err);
    return error.detail.kind === "command" ? error.detail.data.summary : error.message;
  }

  $effect(() => {
    const mode = choice.tags;
    void [choice.local, choice.remote, choice.branch, revision];
    const ticket = ++counted;
    if (mode === "none" || problem !== null) {
      tags = null;
      return;
    }
    tags = { state: "counting" };
    const options = untrack(() => pushOptionsOf(choice));
    const timer = setTimeout(() => {
      pushTagsPreview(request.repo, options)
        .then((names) => {
          if (ticket === counted) tags = { state: "done", tags: names };
        })
        .catch((err: unknown) => {
          if (ticket === counted) tags = { state: "failed", reason: failureLine(err) };
        });
    }, TAG_DELAY_MS);
    return () => clearTimeout(timer);
  });

  let urls = $state<Record<string, string>>({});
  const url = $derived(urls[choice.remote] ?? "");

  onMount(() => {
    for (const name of request.remotes) {
      remoteInfo(request.repo, name)
        .then((info) => (urls[name] = info.pushUrl ?? info.url ?? ""))
        .catch(() => {});
    }
    return () => {
      asked += 1;
      counted += 1;
    };
  });

  const counter = $derived(tagCounter(tags));

  /** What each remote said of `--atomic`, asked once Advanced shows it; `null`: not reached. */
  let atomicSupport = $state<Record<string, boolean | null>>({});
  const atomic = $derived(atomicGate(preview, choice, tags, atomicSupport[choice.remote]));
  /** What goes: no `--set-upstream` that changes nothing, no `--atomic` where it cannot hold. */
  const sent = $derived({ ...effectiveChoice(choice, upstream), atomic: choice.atomic && atomic.enabled });
  const command = $derived(pushCommand(sent, preview?.remoteTip ?? null));

  $effect(() => {
    const remote = choice.remote;
    if (!networkDialog.advancedOpen || problem !== null || remote in untrack(() => atomicSupport)) return;
    const options = untrack(() => pushOptionsOf(choice));
    pushAtomicSupport(request.repo, options)
      .then((answer) => (atomicSupport[remote] = answer))
      .catch(() => (atomicSupport[remote] = null));
  });

  const more = $derived(preview ? preview.total - preview.commits.length : 0);
  const status = $derived(
    preview &&
      statusLine(preview, choice.remote, Date.now(), settings.current.fetchStaleHours, fetchedHere[choice.remote] ?? null),
  );
  /** The status may no longer hold: Fetch comes first, Push waits. */
  const stale = $derived(status?.stale ?? false);

  let fetchButton = $state<HTMLButtonElement>();
  let focusSettled = false;
  /** Once: the footer focused Push while it was the main action, before the status said
      Fetch comes first, and Enter on a focused button presses that button. */
  $effect(() => {
    if (focusSettled || !status) return;
    focusSettled = true;
    if (status.stale && document.activeElement?.closest("footer")) fetchButton?.focus();
  });
  const button = $derived(pushButton(preview, choice, upstream, tags));
</script>

<TemplateDialog
  title="Push commits to a remote repository"
  fill
  {onclose}
  status={problem}
  destructive={danger}
  onenter={stale && !fetching ? () => void fetchRemote() : choice.dryRun ? () => void push() : undefined}
  actions={[
    {
      label: button.label,
      primary: !stale && !choice.dryRun,
      disabled: problem !== null || fetching || button.nothing,
      tip: button.nothing ? "Nothing to push" : undefined,
      onclick: () => void push(),
    },
  ]}
>
  <div class="pinned">
    <div class="fields">
      <div class="field wide">
        <span class="caption">Local branch</span>
        <RevisionCombobox
          value={choice.local}
          label="Local branch"
          free={false}
          options={request.locals.map((b) => ({ value: b.name, label: b.name, group: "" }))}
          onchange={pickLocal}
        />
      </div>
      <div class="field">
        <span class="caption">Remote</span>
        <RevisionCombobox
          value={choice.remote}
          label="Remote"
          free={false}
          tip={url ? `${choice.remote} — ${url}` : choice.remote}
          popupWidth={REMOTE_LIST_WIDTH}
          options={request.remotes.map((name) => ({ value: name, label: name, group: "", hint: urls[name] || undefined }))}
          onchange={chooseRemote}
        />
      </div>
      <div class="field">
        <span class="caption">Remote branch</span>
        <RevisionCombobox
          value={choice.branch}
          label="Remote branch"
          options={branches.map((name) => ({ value: name, label: name, group: "" }))}
          onchange={(next) => (choice.branch = next)}
        />
      </div>
    </div>
    <div class="status-line">
      <span class="status truncate" class:stale title={status?.text}>{status?.text ?? ""}</span>
      <button
        type="button"
        bind:this={fetchButton}
        class="btn sm"
        class:primary={stale}
        title="Fetch {choice.remote} and read again"
        disabled={fetching}
        onclick={() => void fetchRemote()}
      >
        {fetching ? "Fetching…" : "Fetch"}
      </button>
    </div>
  </div>

  <div class="scroll">
    {#if preview && preview.behind > 0 && !choice.forceWithLease}
      <Callout kind="warning">Remote has commits you don't have — push will be rejected. Fetch and integrate first.</Callout>
    {/if}

    <PushList label={commitsText(preview)} empty={!preview || preview.total === 0} open={listOpen} ontoggle={() => (listOpen = !listOpen)}>
      {#each preview?.commits ?? [] as commit (commit.oid)}
        <li>
          <span class="oid mono">{shortOid(commit.oid)}</span>
          <span class="summary truncate" title={commit.summary}>{commit.summary}</span>
          <span class="who">{commit.author.name}</span>
          <span class="who">{settings.formatDate(commit.author.timestamp, commit.author.tzOffsetMinutes)}</span>
        </li>
      {/each}
      {#if more > 0}<li class="rest">and {more} more</li>{/if}
    </PushList>

    <div class="options">
      {#if tracking.kind === "already"}
        <p class="upstream">{tracking.hint}</p>
      {:else}
        <OptionRow bind:checked={choice.setUpstream} label="Set as upstream" tip={tracking.hint} />
      {/if}
      <div class="tags">
        <span>Tags</span>
        <Select value={choice.tags} label="Tags" options={TAG_CHOICES} onchange={(mode) => (choice.tags = mode)} />
        {#if counter}<span class="count" title={counter.tip}>{counter.text}</span>{/if}
      </div>
      <OptionRow bind:checked={choice.notes} label="Push notes" tip="refs/notes/*, never forced" note={notesNote(preview)} />
      <OptionRow
        bind:checked={choice.forceWithLease}
        label="Force (with lease)"
        hint="Overwrite the remote branch unless it moved since your last fetch"
      />
      {#if choice.forceWithLease}
        <div class="nested">
          <OptionRow
            bind:checked={choice.forceIfIncludes}
            label="Only if the remote tip is in your branch's history"
            tip="--force-if-includes"
          />
          <p class="lease"><code title={preview?.remoteTip ?? undefined}>{lease.flag}</code> {lease.note}</p>
          <Callout kind="warning">
            The remote branch will be overwritten with yours: commits it has that you do not are lost there.
            Off again the next time this dialog opens.
          </Callout>
        </div>
      {/if}
      <OptionRow bind:checked={remember} label="Remember as default for repository" tip="Saves remote, tags and notes (Force is never saved)" />
    </div>

    <div class="group">
      <button
        type="button"
        class="caption advanced"
        aria-expanded={networkDialog.advancedOpen}
        onclick={() => void networkDialog.setAdvancedOpen(!networkDialog.advancedOpen)}
      >
        <Disclosure open={networkDialog.advancedOpen} />Advanced
      </button>
      {#if networkDialog.advancedOpen}
        <OptionRow bind:checked={choice.noVerify} label="Skip pre-push hook" flag="--no-verify" tip="Push without running the pre-push hook" />
        <OptionRow
          bind:checked={choice.dryRun}
          label="Dry run"
          flag="--dry-run"
          tip="Show what would be pushed in the output window; nothing is sent"
        />
        <OptionRow
          checked={choice.atomic && atomic.enabled}
          disabled={!atomic.enabled}
          label="Atomic"
          flag="--atomic"
          tip={atomic.tip}
          onchange={(on) => (choice.atomic = on)}
        />
      {/if}
    </div>
  </div>

  <div class="command">
    <span class="caption">Command</span>
    <div class="command-field">
      <input readonly aria-label="Command" spellcheck="false" value={command.short} title={command.full} />
      <CopyButton icon class="copy" label="Copy command" text={command.short} />
    </div>
  </div>
</TemplateDialog>

<style>
  .pinned,
  .scroll {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    min-width: 0;
  }

  .pinned,
  .command {
    flex: none;
  }

  /* Only the options move: the fields, the status and the command stay in view. */
  .scroll {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
  }

  .fields {
    display: grid;
    grid-template-columns: minmax(0, 3fr) minmax(0, 7fr);
    gap: var(--sp-4) var(--sp-5);
  }

  .wide {
    grid-column: 1 / -1;
  }

  .field,
  .group {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }

  /* The label beside the field: a line less under everything else. */
  .command {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    min-width: 0;
  }

  .group {
    gap: var(--sp-3);
  }

  /* A dense list: one line an option, the explanation in the label's tooltip. */
  .options {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
  }

  .tags {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    min-width: 0;
    font-size: var(--fs-dense);
  }

  .count {
    color: var(--text-muted);
    white-space: nowrap;
  }

  .caption {
    color: var(--text-secondary);
    font-size: var(--fs-dense);
    line-height: 1.3;
  }

  .status-line {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
  }

  .status {
    flex: 1 1 auto;
    min-width: 0;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .status.stale {
    color: var(--status-warning);
  }

  .mono,
  code {
    font-family: var(--font-mono);
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }

  code {
    font-size: var(--fs-dense);
    line-height: 1.4;
  }

  /* One line that scrolls sideways, the copy icon inside it on the right. */
  .command-field {
    display: flex;
    flex: 1 1 auto;
    align-items: center;
    min-width: 0;
    height: var(--h-input);
    padding-right: var(--sp-2);
    background: var(--surface-input);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
  }

  .command-field:focus-within {
    outline: 1px solid var(--state-focus-ring);
    outline-offset: -1px;
  }

  .command-field input {
    flex: 1 1 auto;
    min-width: 0;
    height: 100%;
    padding: 0 var(--sp-4);
    background: none;
    border: 0;
    outline: none;
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: var(--fs-dense);
  }

  .command-field :global(.copy) {
    display: flex;
    padding: var(--sp-2);
    background: none;
    border: 0;
    border-radius: var(--r-sm);
    color: var(--text-secondary);
    cursor: default;
  }

  .command-field :global(.copy:hover) {
    background: var(--state-hover);
    color: var(--text-primary);
  }

  .advanced {
    display: inline-flex;
    align-items: center;
    align-self: flex-start;
    padding: 0;
    background: none;
    border: 0;
    font: inherit;
    font-size: var(--fs-dense);
    color: var(--text-secondary);
    cursor: default;
  }

  .upstream {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .nested {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding-left: calc(var(--sp-6) + var(--sp-2));
  }

  .lease {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
    overflow-wrap: anywhere;
  }

  li {
    display: flex;
    gap: var(--sp-4);
    min-width: 0;
  }

  .oid {
    flex: none;
  }

  .summary {
    flex: 1 1 auto;
    min-width: 0;
  }

  .who {
    flex: none;
    color: var(--text-secondary);
  }

  .rest {
    color: var(--text-secondary);
  }
</style>
