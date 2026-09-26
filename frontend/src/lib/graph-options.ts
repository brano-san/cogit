import { COLORING_LABELS, GRAPH_COLORINGS, type GraphColoring } from "$lib/graph-coloring";
import type { Settings } from "$lib/settings";

/** Switches of the options menu that are not graph modes; Preferences shows them as well. */
export const GRAPH_SWITCHES = ["graphSelectedRefsOnly", "graphIncludeTracked", "graphWorkingTreeAlways"] as const;
export type GraphSwitch = (typeof GRAPH_SWITCHES)[number];

/** The switches of the graph's options menu: each is a setting Preferences shows too. */
export type GraphOptionKey = "graphFirstParent" | "graphWhileFiltering" | GraphSwitch;

export type GraphOptionEntry =
  | { kind: "coloring"; coloring: GraphColoring; label: string; hint: string; checked: boolean }
  | { kind: "preferences"; label: string; hint: string }
  | { kind: "switch"; key: GraphOptionKey; label: string; hint: string; checked: boolean }
  | { kind: "separator" };

const SWITCHES: readonly { key: GraphOptionKey; label: string; hint: string }[] = [
  {
    key: "graphFirstParent",
    label: "Follow Only First Parent",
    hint: "One line of history: what was merged in is not listed.",
  },
  {
    key: "graphSelectedRefsOnly",
    label: "Show Only Selected Branches and Tags",
    hint: "Labels only for the refs ticked in Branches.",
  },
  {
    key: "graphIncludeTracked",
    label: "Include Tracked Remote Branches",
    hint: "A branch ticked in Branches brings the remote branch it tracks into the graph.",
  },
  {
    key: "graphWhileFiltering",
    label: "Show Graph While Filtering",
    hint: "A filtered list keeps its lines: each match joins the next one down its first parents.",
  },
];

const WORKING_TREE = {
  key: "graphWorkingTreeAlways",
  label: "Show Working Tree Permanently",
  hint: "Off: the Working Tree row is left out while the working tree has no changes.",
} as const;

/** The menu beside the graph filter, as SmartGit orders it (F-561). The values are the
    settings themselves, so the menu and Preferences never disagree. */
export function graphOptions(settings: Pick<Settings, "graphColoring" | GraphOptionKey>): GraphOptionEntry[] {
  return [
    ...GRAPH_COLORINGS.map((coloring) => ({
      kind: "coloring" as const,
      coloring,
      ...COLORING_LABELS[coloring],
      checked: settings.graphColoring === coloring,
    })),
    { kind: "separator" },
    {
      kind: "preferences",
      label: "Graph Preferences…",
      hint: "Preferences ▸ Graph & History: columns, rows, lanes and the rest of the graph's settings",
    },
    { kind: "separator" },
    ...SWITCHES.map((entry) => ({ kind: "switch" as const, ...entry, checked: settings[entry.key] })),
    { kind: "separator" },
    { kind: "switch", ...WORKING_TREE, checked: settings.graphWorkingTreeAlways },
  ];
}
