import {
  DEFAULT_LAYOUT, DEFAULT_PERSPECTIVES, mergePerspective, mergePerspectives, type Perspective, type PerspectiveId,
} from "./perspectives";

export const WINDOW_LAYOUT_CHOICES = [
  ["standard", "Standard"],
  ["log", "Log"],
  ["workingTree", "Working Tree"],
] as const;
export type WindowLayout = (typeof WINDOW_LAYOUT_CHOICES)[number][0];
export type LayoutProfile = PerspectiveId | Exclude<WindowLayout, "standard">;

interface WindowArrangement {
  defaults: Perspective;
  diff: "below" | "beside";
  files: "selection" | "history" | "worktree";
}

export const WINDOW_LAYOUTS: Record<WindowLayout, WindowArrangement> = {
  standard: { defaults: DEFAULT_PERSPECTIVES.main, diff: "below", files: "selection" },
  log: {
    defaults: {
      fractions: { ...DEFAULT_LAYOUT, leftColumn: 0.2, topRow: 0.62, graph: 0.72 },
      hidden: ["worktrees", "commit"],
    },
    diff: "below",
    files: "history",
  },
  workingTree: {
    defaults: {
      fractions: { ...DEFAULT_LAYOUT, leftColumn: 0.2, topRow: 0.46, commitHeight: 0.21 },
      hidden: ["worktrees", "graph"],
    },
    diff: "beside",
    files: "worktree",
  },
};

export function windowProfiles(stored: unknown, legacy: unknown): Record<LayoutProfile, Perspective> {
  const source = stored && typeof stored === "object" ? stored as Partial<Record<LayoutProfile, unknown>> : {};
  return {
    ...mergePerspectives(source.main || source.review ? source : legacy),
    log: mergePerspective(source.log, WINDOW_LAYOUTS.log.defaults),
    workingTree: mergePerspective(source.workingTree, WINDOW_LAYOUTS.workingTree.defaults),
  };
}

export function workingTreeFiles(layout: WindowLayout, selectedCommit: string | null): boolean {
  const { files } = WINDOW_LAYOUTS[layout];
  return files === "worktree" || (files === "selection" && selectedCommit === null);
}
