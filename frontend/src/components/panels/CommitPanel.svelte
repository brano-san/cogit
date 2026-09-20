<script lang="ts">
  import CommitBox from "$components/file-list/CommitBox.svelte";
  import { repository } from "$stores/repository.svelte";
  import { worktree } from "$stores/worktree.svelte";

  interface Props {
    /** What "Commit What You See" will actually commit, given the file filter. */
    scope: ReturnType<typeof import("$lib/commit-scope").commitScope>;
    template: string | null;
    oncommit: (message: string, amend: boolean, noVerify: boolean) => void;
  }

  let { scope, template, oncommit }: Props = $props();
</script>

<CommitBox
  {scope}
  {template}
  stagedCount={worktree.staged.length}
  busy={worktree.loading}
  draftKey={`cogit:draft:${repository.current?.root ?? ""}`}
  {oncommit}
/>
