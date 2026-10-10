<script lang="ts">
  import { onMount, untrack } from "svelte";
  import PushList from "./PushList.svelte";
  import CopyButton from "$components/common/CopyButton.svelte";
  import RevisionCombobox from "$components/common/RevisionCombobox.svelte";
  import Select from "$components/common/Select.svelte";
  import Callout from "$components/common/template/Callout.svelte";
  import OptionRow from "$components/common/template/OptionRow.svelte";
  import TemplateDialog from "$components/common/template/TemplateDialog.svelte";
  import { shortOid } from "$lib/format";
  import { toCogitError } from "$lib/ipc";
  import { remoteInfo } from "$lib/ipc/remotes";
  import {
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
    tagsHint,
    type PushChoice,
  } from "$lib/network-dialogs";
  import {
    commitsText,
    effectiveChoice,
    fetchedText,
    isUpToDate,
    leaseLine,
    protectedQuestion,
    pushButtonLabel,
    pushCommand,
    pushStatus,
    tagCountText,
    upstreamState,
    type TagCount,
  } from "$lib/push-dialog";
  import { splitUpstream } from "$lib/push-to";
  import { confirmation } from "$stores/confirm.svelte";
  import type { PushRequest } from "$stores/network-dialog.svelte";
  import { settings } from "$stores/settings.svelte";

  /** Push (item 12), the Push dialog: where, what goes, which tags and notes. */
  interface Props {
    request: PushRequest;
    /** `options` is what goes; `choice` is what Remember keeps. */
    onpush: (choice: PushChoice, options: PushOptions, remember: boolean) => void;
    /** Fetches `remote` and refreshes the panels; the dialog then reads everything again. */
    onfetch: (remote: string) => Promise<void>;
    onclose: () => void;
  }

  let { request, onpush, onfetch, onclose }: Props = $props();

  /** The list never holds more; the count above it is exact. */
  const LIST_CAP = 200;
  const TAG_MODES: readonly TagsMode[] = ["none", "follow", "all"];
  const TAG_LABELS: Record<TagsMode, string> = {
    none: "No tags",
    follow: "Tags pointing to pushed commits",
    all: "All tags",
  };

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

  async function fetchRemote() {
    fetching = true;
    try {
      await onfetch(choice.remote);
      revision += 1;
    } finally {
      fetching = false;
    }
  }

  const upstream = $derived(request.locals.find((b) => b.name === choice.local)?.upstream ?? null);
  const branches = $derived(remoteBranchChoices(choice.remote, request.remoteBranches, choice.local));
  const problem = $derived(pushProblem(choice));
  const tracking = $derived(upstreamState(upstream, choice.remote, choice.branch));
  const sent = $derived(effectiveChoice(choice, upstream));
  const command = $derived(pushCommand(sent, preview?.remoteTip ?? null));
  const danger = $derived(choice.forceWithLease);
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
  let tagsOpen = $state(false);
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

  const noteHint = $derived.by(() => {
    if (!preview?.hasLocalNotes) return undefined;
    if (preview.notesUnpushed === null) return "Not compared with the remote: fetch its notes to see";
    return preview.notesUnpushed === 0 ? undefined : `${preview.notesUnpushed} notes not pushed`;
  });

  const more = $derived(preview ? preview.total - preview.commits.length : 0);
  const upToDate = $derived(preview !== null && isUpToDate(preview, choice, upstream, tags));
</script>

<TemplateDialog
  title="Push commits to a remote repository"
  {onclose}
  status={problem}
  destructive={danger}
  actions={[
    {
      label: pushButtonLabel(choice, preview?.total ?? 0),
      primary: true,
      disabled: problem !== null || fetching || upToDate,
      tip: upToDate ? `Nothing to push: ${choice.remote}/${choice.branch} is at the same commit` : undefined,
      onclick: () => void push(),
    },
  ]}
>
  <p class="sub">Sends the commits of {choice.local} to the remote.</p>

  <div class="fields">
    <div class="field">
      <span class="caption">Local branch</span>
      <Select
        value={choice.local}
        label="Local branch"
        options={request.locals.map((b) => [b.name, b.name] as const)}
        onchange={pickLocal}
      />
    </div>
    <div class="field">
      <span class="caption">Remote</span>
      <Select
        value={choice.remote}
        label="Remote"
        options={request.remotes.map((name) => [name, name] as const)}
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
  {#if url}<p class="url" title={url}>{url}</p>{/if}
  <div class="route">
    <div class="where">
      <p><span class="mono">{choice.local}</span> → <span class="mono">{choice.remote}/{choice.branch}</span></p>
      {#if preview}
        <p class="status">{pushStatus(preview, choice.remote)} · {fetchedText(preview.fetchedAt, Date.now())}</p>
      {/if}
    </div>
    <button type="button" class="btn" title="Fetch {choice.remote} and read again" disabled={fetching} onclick={() => void fetchRemote()}>
      {fetching ? "Fetching…" : "Fetch"}
    </button>
  </div>
  {#if preview && preview.behind > 0 && !choice.forceWithLease}
    <Callout kind="warning">Remote has commits you don't have — push will be rejected. Fetch and integrate first.</Callout>
  {/if}

  <PushList label={commitsText(preview)} empty={!preview || preview.total === 0} open={listOpen} ontoggle={() => (listOpen = !listOpen)}>
    {#each preview?.commits ?? [] as commit (commit.oid)}
      <li><span class="oid mono">{shortOid(commit.oid)}</span><span class="summary">{commit.summary}</span></li>
    {/each}
    {#if more > 0}<li class="rest">and {more} more</li>{/if}
  </PushList>

  <div class="group">
    <span class="caption">Tracking</span>
    <OptionRow
      checked={tracking.kind === "already" || choice.setUpstream}
      disabled={tracking.kind === "already"}
      label="Set as upstream"
      hint={tracking.hint}
      onchange={(on) => (choice.setUpstream = on)}
    />
  </div>

  <div class="group">
    <span class="caption">Tags</span>
    {#each TAG_MODES as mode (mode)}
      <OptionRow
        kind="radio"
        name="push-tags"
        label={TAG_LABELS[mode]}
        hint={tagsHint(mode)}
        checked={choice.tags === mode}
        onchange={() => (choice.tags = mode)}
      />
      {#if mode !== "none" && choice.tags === mode && tags !== null}
        <div class="nested">
          <PushList
            label={tagCountText(tags)}
            empty={tags.state !== "done" || tags.tags.length === 0}
            open={tagsOpen}
            ontoggle={() => (tagsOpen = !tagsOpen)}
          >
            {#each tags.state === "done" ? tags.tags : [] as tag (tag)}<li class="mono">{tag}</li>{/each}
          </PushList>
        </div>
      {/if}
    {/each}
  </div>

  <div class="group">
    <span class="caption">Options</span>
    <OptionRow bind:checked={choice.notes} label="Push notes" hint="refs/notes/*, never forced" />
    {#if noteHint}<p class="note">{noteHint}</p>{/if}
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
          hint="--force-if-includes"
        />
        <p class="lease"><code title={preview?.remoteTip ?? undefined}>{lease.flag}</code> {lease.note}</p>
        <Callout kind="warning">
          The remote branch will be overwritten with yours: commits it has that you do not are lost there.
          Off again the next time this dialog opens.
        </Callout>
      </div>
    {/if}
    <OptionRow
      bind:checked={remember}
      label="Remember as default for repository"
      hint="Saves remote, tags, notes and upstream settings (Force is never saved)"
    />
  </div>

  <div class="command">
    <span class="caption">Command</span>
    <div class="line">
      <code title={command.full}>{command.short}</code>
      <CopyButton class="btn" text={command.short} />
    </div>
  </div>
</TemplateDialog>

<style>
  .fields {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-5);
  }

  .field,
  .group,
  .command {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }

  .group {
    gap: var(--sp-3);
  }

  .caption {
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .route {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-4);
  }

  .where {
    flex: 1 1 auto;
    min-width: 0;
  }

  .where p {
    margin: 0;
    font-size: var(--fs-dense);
    color: var(--text-secondary);
    overflow-wrap: anywhere;
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

  .line {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
  }

  .line code {
    flex: 1 1 auto;
    min-width: 0;
  }

  .url {
    margin: 0;
    color: var(--text-muted);
    font-size: var(--fs-dense);
    overflow-wrap: anywhere;
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

  .note {
    margin: 0;
    padding-left: calc(var(--sp-6) + var(--sp-2));
    color: var(--text-secondary);
    font-size: var(--fs-dense);
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
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .rest {
    color: var(--text-secondary);
  }
</style>
