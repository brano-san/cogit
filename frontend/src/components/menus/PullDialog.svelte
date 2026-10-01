<script lang="ts">
  import { onMount } from "svelte";
  import Disclosure from "$components/common/Disclosure.svelte";
  import Select from "$components/common/Select.svelte";
  import OptionRow from "$components/common/template/OptionRow.svelte";
  import TemplateDialog from "$components/common/template/TemplateDialog.svelte";
  import { remoteInfo } from "$lib/ipc/remotes";
  import { pullChoiceOf, pullCommandLine, type PullChoice } from "$lib/network-dialogs";
  import { networkDialog, type PullRequest } from "$stores/network-dialog.svelte";

  /** Pull and Fetch Only (item 12), after SmartGit's Pull dialog. */
  interface Props {
    request: PullRequest;
    onrun: (action: "pull" | "fetch", remote: string, choice: PullChoice, remember: boolean) => void;
    onclose: () => void;
  }

  let { request, onrun, onclose }: Props = $props();

  // svelte-ignore state_referenced_locally
  let remote = $state(request.remote);
  // svelte-ignore state_referenced_locally
  let choice = $state<PullChoice>(pullChoiceOf(request.defaults));
  let remember = $state(false);
  let urls = $state<Record<string, string>>({});

  onMount(() => {
    for (const name of request.remotes) {
      remoteInfo(request.repo, name)
        .then((info) => (urls[name] = info.url ?? ""))
        .catch(() => {});
    }
  });

  const command = $derived(pullCommandLine(remote, choice, request.ffOnly));
  const url = $derived(urls[remote] ?? "");
</script>

<TemplateDialog
  title="Pull commits from a remote repository"
  {onclose}
  actions={[
    {
      label: "Fetch Only",
      tip: "Updates the remote-tracking branches and tags; your branch is not touched",
      onclick: () => onrun("fetch", remote, choice, remember),
    },
    { label: "Pull", primary: true, onclick: () => onrun("pull", remote, choice, remember) },
  ]}
>
  <p class="sub">
    Pull fetches the commits of the remote and integrates them into the current branch. Fetch Only only
    updates what is known of the remote.
  </p>

  <div class="field">
    <span class="caption">Fetch From</span>
    <Select
      value={remote}
      label="Fetch From"
      options={request.remotes.map((name) => [name, name] as const)}
      onchange={(next) => (remote = next)}
    />
    {#if url}<span class="url">{url}</span>{/if}
  </div>

  <div class="more">
    <button
      type="button"
      class="toggle"
      aria-expanded={networkDialog.moreOpen}
      onclick={() => void networkDialog.setMoreOpen(!networkDialog.moreOpen)}
    >
      <Disclosure open={networkDialog.moreOpen} />
      <span>More Options</span>
    </button>
    {#if networkDialog.moreOpen}
      <div class="options">
        <OptionRow
          kind="radio"
          name="pull-method"
          label="Merge fetched remote changes"
          checked={choice.method === "merge"}
          onchange={() => (choice.method = "merge")}
        />
        <OptionRow
          kind="radio"
          name="pull-method"
          label="Rebase local branch onto fetched changes"
          checked={choice.method === "rebase"}
          onchange={() => (choice.method = "rebase")}
        />
        <OptionRow
          bind:checked={choice.tags}
          label="Update existing and fetch new tags"
          hint="Tags that moved on the remote are moved here too (--tags --force)"
        />
        <OptionRow
          bind:checked={choice.notes}
          label="Fetch notes"
          hint="refs/notes/*; notes that diverged are never overwritten, a merge is offered"
        />
        <OptionRow bind:checked={remember} label="Remember as default for repository" />
      </div>
    {/if}
  </div>

  <div class="command"><span class="caption">Command</span><code>{command}</code></div>
</TemplateDialog>

<style>
  .field,
  .command {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }

  .caption {
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .url,
  code {
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: var(--fs-dense);
    line-height: 1.4;
    overflow-wrap: anywhere;
  }

  .more {
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
    cursor: default;
  }

  .options {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding-left: var(--sp-4);
  }
</style>
