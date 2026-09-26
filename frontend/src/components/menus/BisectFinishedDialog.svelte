<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import { dateTooltip } from "$lib/format";
  import type { CommitDetails } from "$lib/ipc";

  /** SmartGit's Bisect Finished: the commit, Copy ID, Continue Bisect and Leave Bisect. */
  interface Props {
    found: CommitDetails;
    term: string;
    busy: boolean;
    oncopy: () => void;
    onleave: () => void;
    onclose: () => void;
  }

  let { found, term, busy, oncopy, onleave, onclose }: Props = $props();
</script>

<Dialog title="Bisect Finished" {onclose} onconfirm={onleave} width="min(560px, 92vw)">
  <div class="form">
    <p class="lead">The first {term} commit is</p>
    <dl>
      <dt>ID</dt>
      <dd class="mono">{found.oid}</dd>
      <dt>Author</dt>
      <dd>
        {found.author.name} &lt;{found.author.email}&gt;,
        {dateTooltip(found.author.timestamp, found.author.tzOffsetMinutes)}
      </dd>
      <dt>Message</dt>
      <dd class="message">{found.body ? `${found.summary}\n\n${found.body}` : found.summary}</dd>
    </dl>
    <p class="hint">
      Leave Bisect checks out the branch the bisect began on; Continue Bisect keeps the marks, to
      test more commits.
    </p>
  </div>

  {#snippet footer()}
    <button type="button" class="btn" onclick={oncopy}>Copy ID</button>
    <span class="grow"></span>
    <button type="button" class="btn" onclick={onclose}>Continue Bisect</button>
    <button type="button" class="btn primary" data-autofocus disabled={busy} onclick={onleave}>
      Leave Bisect
    </button>
  {/snippet}
</Dialog>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
    font-size: var(--fs-dense);
  }

  .lead,
  .hint {
    margin: 0;
    line-height: 1.5;
  }

  .hint,
  dt {
    color: var(--text-secondary);
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--sp-3) var(--sp-5);
    margin: 0;
  }

  dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
    user-select: text;
  }

  .message {
    max-height: 160px;
    overflow: auto;
    white-space: pre-wrap;
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
