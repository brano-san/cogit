<script lang="ts">
  import { shortOid } from "$lib/format";
  import { commit } from "$stores/commit.svelte";
  import { repository } from "$stores/repository.svelte";
  import { settings } from "$stores/settings.svelte";

  /** What the Diff panel shows when there is no file to diff: the selected commit, the
      error that stopped one from loading, or what to do next. */
  interface Props {
    oncherrypick: () => void;
    onrevert: () => void;
    onsplit: () => void;
    onrebase: () => void;
    onrollback: () => void;
  }

  let { oncherrypick, onrevert, onsplit, onrebase, onrollback }: Props = $props();

  const repo = $derived(repository.current);
  const details = $derived(commit.details);
</script>

<div class="detail">
  {#if repository.error}
    <p class="error">{repository.error.message}</p>
    {#if repository.error.isCommandFailure && repository.error.detail.kind === "command"}
      <pre class="raw">{repository.error.detail.data.stderr}</pre>
    {/if}
  {:else if commit.error}
    <p class="error">{commit.error.message}</p>
  {:else if details}
    <p class="subject">{details.summary}</p>
    {#if details.body}<pre class="body">{details.body}</pre>{/if}
    <dl>
      <dt>Commit</dt>
      <dd class="mono">{details.oid}</dd>
      <dt>Author</dt>
      <dd>
        {details.author.name} &lt;{details.author.email}&gt; ·
        {settings.formatDate(details.author.timestamp, details.author.tzOffsetMinutes)}
      </dd>
      <dt>Parents</dt>
      <dd class="mono tabular">
        {details.parents.length === 0
          ? "none (root commit)"
          : details.parents.map(shortOid).join(", ")}
      </dd>
    </dl>
    <div class="commit-actions">
      <button type="button" onclick={oncherrypick}>Cherry-pick</button>
      <button type="button" onclick={onrevert}>Revert</button>
      <button type="button" onclick={onsplit}>Split Off…</button>
      <button type="button" onclick={onrebase}>Rebase…</button>
      <button type="button" onclick={onrollback}>Roll Back Tree</button>
    </div>
  {:else if repo}
    <dl>
      <dt>Repository</dt>
      <dd class="mono">{repo.root}</dd>
      <dt>HEAD</dt>
      <dd class="mono">{repository.headLabel}</dd>
      <dt>Branches</dt>
      <dd class="mono tabular">
        {repository.localBranches.length} local, {repository.remoteBranches.length} remote
      </dd>
    </dl>
    <p class="muted">Select a commit to see what it changed.</p>
  {:else}
    <p class="muted">No repository open.</p>
  {/if}
</div>

<style>
  .detail {
    padding: var(--sp-5);
    font-size: var(--fs-dense);
    overflow: auto;
  }

  .subject {
    margin: 0 0 var(--sp-4);
    font-weight: 600;
  }

  .body,
  .raw {
    margin: 0 0 var(--sp-5);
    padding: var(--sp-4);
    background: var(--surface-input);
    border-radius: var(--r-sm);
    font-family: var(--font-mono);
    font-size: var(--fs-code);
    white-space: pre-wrap;
    user-select: text;
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--sp-3) var(--sp-5);
    margin: 0;
  }

  dt {
    color: var(--text-secondary);
  }

  dd {
    margin: 0;
    user-select: text;
  }

  .error {
    margin: 0 0 var(--sp-4);
    color: var(--status-delete);
    user-select: text;
  }

  .muted {
    margin: var(--sp-5) 0 0;
    color: var(--text-secondary);
  }

  /* The one that stands alone gets the placeholder shape the panels share. */
  .detail > .muted:only-child {
    margin: 0;
    padding: var(--sp-7) var(--sp-5);
    text-align: center;
    font-size: var(--fs-dense);
  }

  .commit-actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-3);
    margin-top: var(--sp-5);
  }
</style>
