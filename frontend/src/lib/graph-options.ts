import { COLORING_LABELS, GRAPH_COLORINGS, type GraphColoring } from "$lib/graph-coloring";
import type { Settings } from "$lib/settings";

/** The switches of the graph's options menu: each is a setting Preferences shows too. */
export type GraphOptionKey = "graphFirstParent";

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
];

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
  ];
}
