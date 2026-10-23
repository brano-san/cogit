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
    /** A repository or submodule with uncommitted changes: the icon fills red (F-451). */
    changed?: boolean;
  }

  let { kind, title, changed = false }: Props = $props();
</script>

<span
  class="kind {kind}"
  class:changed
  title={title ?? KIND_LABELS[kind]}
  role="img"
  aria-label={changed ? `${KIND_LABELS[kind]} with uncommitted changes` : KIND_LABELS[kind]}
>
  <svg viewBox="0 0 16 16" aria-hidden="true">
    {#if kind === "repository" || kind === "submodule"}
      <!-- A stack of three disks, as SmartGit draws a repository; a submodule is one too. -->
      <rect class="body" x="1.75" y="2.25" width="12.5" height="11.5" rx="1.5" />
      <path class="disks" d="M1.75 6.1h12.5M1.75 9.9h12.5" />
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

  .repository,
  .submodule,
  .worktree {
    color: var(--status-ref);
  }

  .kind.changed {
    color: var(--indicator-repo-changes);
  }

  .changed .body {
    fill: currentColor;
  }

  .changed .disks {
    stroke: var(--text-on-accent);
  }
</style>
