import {
  COLORING_CONFLICTS as BY_COLORING,
  MODE_CONFLICTS as CONFLICTS,
  type GraphMode as Mode,
} from "$lib/graph-modes";
import type { Settings } from "$lib/settings";

/** The graph modes of Preferences ▸ Graph & History, and the ticked-branch colours they
    have to get along with. */
export const GRAPH_MODES = ["graphFirstParent", "graphAncestry", "graphCollapseMerged", "graphWhileFiltering"] as const;

export type GraphMode = (typeof GRAPH_MODES)[number];
export type ModeKey = GraphMode | "graphHighlightChecked";

/** `mode` does nothing while `by` is on, so its switch is greyed out with `reason`. */
export interface ModeConflict {
  mode: ModeKey;
  by: ModeKey;
  reason: string;
}

export interface BlockedMode {
  mode: ModeKey;
  reason: string;
}

const KEYS: Record<Mode, ModeKey> = {
  highlightChecked: "graphHighlightChecked",
  firstParent: "graphFirstParent",
  ancestry: "graphAncestry",
  collapseMerged: "graphCollapseMerged",
  filteredGraph: "graphWhileFiltering",
};

/** The graph's own table (`$lib/graph-modes`), in settings keys: one table for both. */
export const MODE_CONFLICTS: readonly ModeConflict[] = CONFLICTS.map(({ mode, by, reason }) => ({
  mode: KEYS[mode],
  by: KEYS[by],
  reason,
}));

/** Only a switch that is off is blocked: one already on stays live, so a conflict read
    from an older file can still be undone from here. */
export function blockedModes(
  state: Pick<Settings, ModeKey | "graphColoring">,
  table: readonly ModeConflict[] = MODE_CONFLICTS,
): BlockedMode[] {
  const blocked: BlockedMode[] = [];
  const block = (mode: ModeKey, reason: string) => {
    if (state[mode] || blocked.some((entry) => entry.mode === mode)) return;
    blocked.push({ mode, reason });
  };
  for (const rule of table) if (state[rule.by]) block(rule.mode, rule.reason);
  for (const rule of BY_COLORING) if (state.graphColoring === rule.coloring) block(KEYS[rule.mode], rule.reason);
  return blocked;
}
