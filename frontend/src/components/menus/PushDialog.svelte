<script lang="ts">
  import { onMount } from "svelte";
  import Disclosure from "$components/common/Disclosure.svelte";
  import RevisionCombobox from "$components/common/RevisionCombobox.svelte";
  import Select from "$components/common/Select.svelte";
  import Callout from "$components/common/template/Callout.svelte";
  import OptionRow from "$components/common/template/OptionRow.svelte";
  import TemplateDialog from "$components/common/template/TemplateDialog.svelte";
  import { shortOid } from "$lib/format";
  import { pushPreview, type PushPreview, type TagsMode } from "$lib/ipc/network-dialogs";
  import {
    pushChoiceOf,
    pushCommandLine,
    pushProblem,
    remoteBranchChoices,
    tagsHint,
    type PushChoice,
  } from "$lib/network-dialogs";
  import { splitUpstream } from "$lib/push-to";
  import type { PushRequest } from "$stores/network-dialog.svelte";

  /** Push (item 12), the Push dialog: where, what goes, which tags and notes. */
  interface Props {
    request: PushRequest;
    onpush: (choice: PushChoice, remember: boolean) => void;
    onclose: () => void;
  }

  let { request, onpush, onclose }: Props = $props();

  /** The list never holds more; the count above it is exact. */
  const LIST_CAP = 200;
  const TAG_MODES: readonly TagsMode[] = ["none", "follow", "all"];
  const TAG_LABELS: Record<TagsMode, string> = {
    none: "No tags",
    follow: "Tags pointing to pushed commits",
    all: "All tags",
  };

  // svelte-ignore state_referenced_locally
  let choice = $state<PushChoice>(
    pushChoiceOf(request.defaults, {
      remote: request.remote,
      local: request.local,
      branch: request.branch,
      hasUpstream: request.upstream !== null,
    }),
  );
  let remember = $state(false);
  let preview = $state<PushPreview | null>(null);
  let listOpen = $state(false);
  let asked = 0;

  const branches = $derived(remoteBranchChoices(choice.remote, request.remoteBranches, request.local));
  const problem = $derived(pushProblem(choice));
  const command = $derived(pushCommandLine(choice));

  function chooseRemote(next: string) {
    choice.remote = next;
    // The branch of the same name on the new remote, as a first push would send it.
    const same = splitUpstream(request.upstream ?? "", request.remotes);
    choice.branch = same && same.remote === next ? same.branch : request.local;
  }

  $effect(() => {
    const { remote, branch } = choice;
    if (problem !== null) return;
    const ticket = ++asked;
    pushPreview(request.repo, request.local, remote, branch, LIST_CAP)
      .then((found) => {
        if (ticket === asked) preview = found;
      })
      .catch(() => {
        if (ticket === asked) preview = null;
      });
  });

  onMount(() => () => void asked++);

  const noteHint = $derived.by(() => {
    if (!preview?.hasLocalNotes) return undefined;
    if (preview.notesUnpushed === null) return "Not compared with the remote: fetch its notes to see";
    return preview.notesUnpushed === 0 ? undefined : `${preview.notesUnpushed} notes not pushed`;
  });

  const more = $derived(preview ? preview.total - preview.commits.length : 0);
</script>

<TemplateDialog
  title="Push commits to a remote repository"
  {onclose}
  status={problem}
  actions={[
    { label: "Push", primary: true, disabled: problem !== null, onclick: () => onpush(choice, remember) },
  ]}
>
  <p class="sub">Sends the commits of the current branch to the remote.</p>

  <div class="fields">
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
      <span class="caption">Branch</span>
      <RevisionCombobox
        value={choice.branch}
        label="Remote branch"
        free={false}
        options={branches.map((name) => ({ value: name, label: name, group: "" }))}
        onchange={(next) => (choice.branch = next)}
      />
    </div>
  </div>
  <p class="route">
    <span class="mono">{choice.local}</span> → <span class="mono">{choice.remote}/{choice.branch}</span>
  </p>

  <div class="commits">
    <button
      type="button"
      class="toggle"
      aria-expanded={listOpen}
      disabled={!preview || preview.total === 0}
      onclick={() => (listOpen = !listOpen)}
    >
      <Disclosure open={listOpen} empty={!preview || preview.total === 0} />
      <span>
        {#if preview === null}Counting commits…
        {:else if preview.total === 0}No commits to push
        {:else}{preview.total} {preview.total === 1 ? "commit" : "commits"} to push{/if}
      </span>
    </button>
    {#if listOpen && preview}
      <ul class="list">
        {#each preview.commits as commit (commit.oid)}
          <li><span class="oid mono">{shortOid(commit.oid)}</span><span class="summary">{commit.summary}</span></li>
        {/each}
        {#if more > 0}<li class="rest">and {more} more</li>{/if}
      </ul>
    {/if}
  </div>

  <OptionRow
    bind:checked={choice.setUpstream}
    label="Set as upstream"
    hint={request.upstream === null
      ? "The branch has no upstream yet (--set-upstream)"
      : `Track ${choice.remote}/${choice.branch} instead of ${request.upstream} (--set-upstream)`}
  />

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
    {/each}
  </div>

  <div class="group">
    <OptionRow bind:checked={choice.notes} label="Push notes" hint="refs/notes/*, never forced" />
    {#if noteHint}<p class="note">{noteHint}</p>{/if}
  </div>

  <OptionRow
    bind:checked={choice.forceWithLease}
    label="Force (with lease)"
    hint="Overwrite the remote branch unless it moved since your last fetch"
  />
  {#if choice.forceWithLease}
    <Callout kind="warning">
      The remote branch will be overwritten with yours: commits it has that you do not are lost there.
      Off again the next time this dialog opens.
    </Callout>
  {/if}

  <OptionRow bind:checked={remember} label="Remember as default for repository" hint="Everything but Force" />

  <div class="command"><span class="caption">Command</span><code>{command}</code></div>
</TemplateDialog>

<style>
  .fields {
    display: grid;
    grid-template-columns: 1fr 1fr;
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

  .note {
    margin: 0;
    padding-left: calc(var(--sp-6) + var(--sp-2));
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .commits {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }

  .toggle {
    display: inline-flex;
    align-items: center;
    align-self: flex-start;
    padding: 0;
    background: none;
    border: 0;
    color: var(--text-primary);
    font: inherit;
    font-size: var(--fs-dense);
    text-align: left;
    cursor: default;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    max-height: 160px;
    margin: 0;
    padding: var(--sp-3) var(--sp-4);
    overflow-y: auto;
    background: var(--bg-panel);
    border-radius: var(--r-sm);
    list-style: none;
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
