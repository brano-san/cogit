import { MODE_CONFLICTS as CONFLICTS, type GraphMode as Mode } from "$lib/graph-modes";
import type { Settings } from "$lib/settings";

/** The graph modes of Preferences ▸ Graph & History, and the ticked-branch colours they
    have to get along with. */
export const GRAPH_MODES = [
  "graphFirstParent",
  "graphBranchOfCommit",
  "graphAncestry",
  "graphCollapseMerged",
] as const;

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
  branchOfCommit: "graphBranchOfCommit",
  ancestry: "graphAncestry",
  collapseMerged: "graphCollapseMerged",
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
  state: Pick<Settings, ModeKey>,
  table: readonly ModeConflict[] = MODE_CONFLICTS,
): BlockedMode[] {
  const blocked: BlockedMode[] = [];
  for (const rule of table) {
    if (state[rule.mode] || !state[rule.by]) continue;
    if (blocked.some((entry) => entry.mode === rule.mode)) continue;
    blocked.push({ mode: rule.mode, reason: rule.reason });
  }
  return blocked;
}
