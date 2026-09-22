<script lang="ts" module>
  export type Kind =
    | "repository"
    | "submodule"
    | "worktree"
    | "group"
    | "file"
    | "symlink"
    | "directory";

  export const KIND_LABELS: Record<Kind, string> = {
    repository: "Repository",
    submodule: "Submodule",
    worktree: "Worktree",
    group: "Group",
    file: "File",
    symlink: "Symbolic link",
    directory: "Directory",
  };
</script>

<script lang="ts">
  /** What a row is, drawn the same in every panel: Files, Repositories, Worktrees (R-180). */
  interface Props {
    kind: Kind;
    /** Replaces the kind's name in the tooltip. */
    title?: string;
  }

  let { kind, title }: Props = $props();
</script>

<span class="kind {kind}" title={title ?? KIND_LABELS[kind]} role="img" aria-label={KIND_LABELS[kind]}>
  <svg viewBox="0 0 16 16" aria-hidden="true">
    {#if kind === "repository"}
      <path d="M4 2.5h8.5v11H4a1.5 1.5 0 0 1-1.5-1.5V4A1.5 1.5 0 0 1 4 2.5Z" />
      <path d="M2.5 11.5A1.5 1.5 0 0 1 4 10h8.5M6 5.5h4" />
    {:else if kind === "submodule"}
      <path d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3l1.5 1.5H13A1.5 1.5 0 0 1 14.5 5.5V12A1.5 1.5 0 0 1 13 13.5H3A1.5 1.5 0 0 1 1.5 12Z" />
      <rect class="solid" x="6" y="7" width="4.5" height="4" rx="0.5" />
    {:else if kind === "worktree"}
      <circle cx="4.5" cy="3.5" r="1.5" />
      <circle cx="4.5" cy="12.5" r="1.5" />
      <circle cx="11.5" cy="5.5" r="1.5" />
      <path d="M4.5 5v6M11.5 7c0 2.5-3 3-7 4" />
    {:else if kind === "group"}
      <path d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3l1.5 1.5H13A1.5 1.5 0 0 1 14.5 5.5V12A1.5 1.5 0 0 1 13 13.5H3A1.5 1.5 0 0 1 1.5 12Z" />
    {:else if kind === "directory"}
      <path class="solid" d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3l1.5 1.5H13A1.5 1.5 0 0 1 14.5 5.5V12A1.5 1.5 0 0 1 13 13.5H3A1.5 1.5 0 0 1 1.5 12Z" />
    {:else if kind === "symlink"}
      <path d="M4 1.5h5l3.5 3.5v9.5H4Z" />
      <path d="M6 12V9.5A1.5 1.5 0 0 1 7.5 8H10M8.5 6.5 10 8 8.5 9.5" />
    {:else}
      <path d="M4 1.5h5l3.5 3.5v9.5H4Z" />
      <path d="M9 1.5V5h3.5" />
    {/if}
  </svg>
</span>

<style>
  .kind {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--kind-icon);
    height: var(--kind-icon);
    color: var(--text-secondary);
  }

  svg {
    width: 100%;
    height: 100%;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .solid {
    fill: currentColor;
    stroke: none;
  }

  .submodule,
  .worktree {
    color: var(--status-ref);
  }
</style>
