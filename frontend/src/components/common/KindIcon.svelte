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
  import { GLYPHS, type Glyph, type GlyphName } from "$lib/icons";

  /** What a row is, drawn the same in every panel: Files, Repositories, Worktrees (R-180). */
  interface Props {
    kind: Kind;
    /** Replaces the kind's name in the tooltip. */
    title?: string;
    /** A repository or submodule with uncommitted changes: the icon fills red (F-451). */
    changed?: boolean;
  }

  let { kind, title, changed = false }: Props = $props();

  const GLYPH_OF: Record<Kind, GlyphName> = {
    repository: "repository",
    submodule: "repository",
    worktree: "worktree",
    group: "folder",
    file: "file",
    symlink: "symlink",
    directory: "folder",
  };

  const shape: Glyph = $derived(GLYPHS[GLYPH_OF[kind]]);
  const disks = $derived(kind === "repository" || kind === "submodule");
</script>

<span
  class="kind {kind}"
  class:changed
  title={title ?? KIND_LABELS[kind]}
  role="img"
  aria-label={changed ? `${KIND_LABELS[kind]} with uncommitted changes` : KIND_LABELS[kind]}
>
  <svg viewBox="0 0 16 16" aria-hidden="true">
    <path class:disk={disks} class:solid={kind === "directory"} d={shape.d} />
    {#if shape.line}<path d={shape.line} />{/if}
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

  .disk {
    stroke-width: 1;
    fill: currentColor;
    fill-opacity: 0.22;
  }

  .kind.changed {
    color: var(--indicator-repo-changes);
  }

  .changed .disk {
    fill-opacity: 1;
  }
</style>
