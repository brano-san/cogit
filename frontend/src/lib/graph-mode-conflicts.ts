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

/** Two switches that cannot both be on; either one being on greys out the other. */
export interface ModeConflict {
  a: ModeKey;
  b: ModeKey;
  reason: string;
}

export interface BlockedMode {
  mode: ModeKey;
  reason: string;
}

/** Placeholder: the real table comes with the modes themselves. */
export const MODE_CONFLICTS: readonly ModeConflict[] = [];

/** Only a switch that is off is blocked: one already on stays live, so a conflict read
    from an older file can still be undone from here. */
export function blockedModes(
  state: Pick<Settings, ModeKey>,
  table: readonly ModeConflict[] = MODE_CONFLICTS,
): BlockedMode[] {
  const blocked: BlockedMode[] = [];
  for (const rule of table) {
    for (const [mode, other] of [
      [rule.a, rule.b],
      [rule.b, rule.a],
    ] as const) {
      if (state[mode] || !state[other]) continue;
      if (blocked.some((entry) => entry.mode === mode)) continue;
      blocked.push({ mode, reason: rule.reason });
    }
  }
  return blocked;
}
