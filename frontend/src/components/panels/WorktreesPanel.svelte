<script lang="ts">
  import WorktreeList from "$components/repo-tree/WorktreeList.svelte";
  import type { WorktreeEntry } from "$lib/ipc";
  import { worktrees } from "$stores/worktrees.svelte";

  interface Props {
    onopen: (entry: WorktreeEntry) => void;
    oncontext: (entry: WorktreeEntry, x: number, y: number) => void;
    onprune: (entry: WorktreeEntry) => void;
    onrepair: (entry: WorktreeEntry) => void;
    onadd: () => void;
  }

  let { onopen, oncontext, onprune, onrepair, onadd }: Props = $props();
</script>

<WorktreeList
  entries={worktrees.entries}
  selected={worktrees.selected}
  onselect={(entry) => (worktrees.selected = entry.path)}
  {onopen}
  {oncontext}
  {onprune}
  {onrepair}
  {onadd}
/>
