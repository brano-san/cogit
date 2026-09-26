/** How the graph colours its lines: SmartGit's four colourings (F-561, R-574). */
export const GRAPH_COLORINGS = ["default", "branch", "mergeable", "varying"] as const;
export type GraphColoring = (typeof GRAPH_COLORINGS)[number];

export const COLORING_LABELS: Record<GraphColoring, { label: string; hint: string }> = {
  default: {
    label: "Default Coloring",
    hint: "The main line bright, the rest grey, branches ticked in Branches in their own colors.",
  },
  branch: {
    label: "Branch Coloring",
    hint: "The branch of the selected commit, or of the line clicked, is drawn in front.",
  },
  mergeable: {
    label: "Mergeable Coloring",
    hint: "What merging the selected commit into HEAD would bring stands out; the rest is dimmed.",
  },
  varying: {
    label: "Varying Coloring",
    hint: "Every line in a color of its own.",
  },
};

/** A settings file from before the colorings kept two switches for what are two of them. */
export function migratedColoring(stored: Record<string, unknown>): GraphColoring | undefined {
  if (stored.graphColoring !== undefined) return undefined;
  if (stored.coloredLanes === true) return "varying";
  if (stored.graphBranchOfCommit === true) return "branch";
  return undefined;
}
