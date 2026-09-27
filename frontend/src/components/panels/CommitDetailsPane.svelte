<script lang="ts">
  import Avatar from "$components/common/Avatar.svelte";
  import { dateTooltip, shortOid } from "$lib/format";
  import { idleMessage, panelView } from "$lib/repo-phase";
  import { commit } from "$stores/commit.svelte";
  import { repository } from "$stores/repository.svelte";
  import { settings } from "$stores/settings.svelte";
  import { commitSignature, type SignatureCheck } from "$lib/ipc";
  import { signatureLabel } from "$lib/signature";

  /** What the Diff panel shows when there is no file to diff: the selected commit, or
      what to do next. Why a repository would not open is told once, in the notification
      (doc/12-risks.md, R-99) — not here as well. What to do with the commit is the graph's
      context menu, not buttons here. */

  const repo = $derived(repository.current);
  const view = $derived(panelView(repository.phase));
  const details = $derived(commit.details);

  /** Checked on request: gpg or ssh-keygen may be slow, or ask for nothing but still spawn. */
  let signature = $state<{ oid: string; check: SignatureCheck } | null>(null);
  let checking = $state(false);
  async function verify(oid: string) {
    if (!repo) return;
    checking = true;
    try {
      signature = { oid, check: await commitSignature(repo.repo, oid) };
    } catch (err) {
      signature = { oid, check: { status: "E", signer: "", key: "", raw: String(err) } };
    } finally {
      checking = false;
    }
  }
</script>

<div class="detail">
  {#if commit.error}
    <p class="error">{commit.error.message}</p>
  {:else if details}
    <p class="subject">{details.summary}</p>
    {#if details.body}<pre class="body">{details.body}</pre>{/if}
    {#each details.notes as note (note.namespace)}
      <p class="note-head">Notes ({note.namespace})</p>
      <pre class="body">{note.text}</pre>
    {/each}
    <dl>
      <dt>Commit</dt>
      <dd class="mono">{details.oid}</dd>
      <dt>Author</dt>
      <dd class="author">
        <Avatar name={details.author.name} email={details.author.email} size={20} />
        <span>
          {details.author.name} &lt;{details.author.email}&gt; ·
          <span title={dateTooltip(details.author.timestamp, details.author.tzOffsetMinutes)}
            >{settings.formatDate(details.author.timestamp, details.author.tzOffsetMinutes)}</span
          >
        </span>
      </dd>
      {#each details.trailers as trailer, at (at)}
        <dt>{trailer.key}</dt>
        <dd>{trailer.value}</dd>
      {/each}
      {#if details.signed}
        <dt>Signature</dt>
        <dd>
          {#if signature?.oid === details.oid}
            <span title={signature.check.raw}>{signatureLabel(signature.check)}</span>
          {:else}
            <button type="button" disabled={checking} onclick={() => verify(details.oid)}>Verify</button>
          {/if}
        </dd>
      {/if}
      {#if details.encoding}
        <dt>Encoding</dt>
        <dd title="The message is not UTF-8 and was decoded from this encoding">{details.encoding}</dd>
      {/if}
      <dt>Parents</dt>
      <dd class="mono tabular">
        {details.parents.length === 0
          ? "none (root commit)"
          : details.parents.map(shortOid).join(", ")}
      </dd>
    </dl>
  {:else if repo && commit.oid === null}
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
    {#if idleMessage(view)}<p class="muted">{idleMessage(view)}</p>{/if}
  {/if}
</div>

<style>
  .author {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
  }

  .detail {
    padding: var(--sp-5);
    font-size: var(--fs-dense);
    overflow: auto;
  }

  .subject {
    margin: 0 0 var(--sp-4);
    font-weight: 600;
    user-select: text;
  }

  .body {
    margin: 0 0 var(--sp-5);
    padding: var(--sp-4);
    background: var(--surface-input);
    border-radius: var(--r-sm);
    font-family: var(--font-mono);
    font-size: var(--fs-code);
    white-space: pre-wrap;
    user-select: text;
  }

  .note-head {
    margin: 0 0 var(--sp-2);
    color: var(--text-secondary);
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
</style>
