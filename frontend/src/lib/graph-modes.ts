import { branchSlot } from "$lib/graph-style";
import type { Branch, GraphPaintRequest, GraphView } from "$lib/ipc";

/** The graph settings this module reads, by the parameter names of the settings contract
    (`graphHighlightChecked` → `highlightChecked`, …). */
export interface GraphModes {
  highlightChecked: boolean;
  /** `--first-parent`: one line per ticked ref, merged branches left out. */
  firstParent: boolean;
  /** A click on a commit or its line brings its branch forward. */
  branchOfCommit: boolean;
  /** Everything but the chosen commit's ancestors and descendants is dimmed. */
  ancestry: boolean;
  /** A branch merged in is one row at its merge, opened with a button there. */
  collapseMerged: boolean;
}

export type GraphMode = keyof GraphModes;

/** As the graph looked before the settings existed, plus colour for ticked branches. */
export const GRAPH_MODE_DEFAULTS: Readonly<GraphModes> = {
  highlightChecked: true,
  firstParent: false,
  branchOfCommit: false,
  ancestry: false,
  collapseMerged: false,
};

/** Modes that cannot both apply: `mode` does nothing while `by` is on. Every other pair
    works together, and with the colours of ticked branches. */
export const MODE_CONFLICTS: readonly { mode: GraphMode; by: GraphMode; reason: string }[] = [
  {
    mode: "collapseMerged",
    by: "firstParent",
    reason: "First parents only already leaves every merged branch out.",
  },
];

/** The modes that are inactive under `modes`, and why: Preferences greys them out. */
export function conflictingModes(modes: GraphModes): { mode: GraphMode; reason: string }[] {
  return MODE_CONFLICTS.filter((conflict) => modes[conflict.by]).map(({ mode, reason }) => ({ mode, reason }));
}

/** `modes` as the graph applies them: an inactive mode is off. */
export function effectiveModes(modes: GraphModes): GraphModes {
  const effective = { ...modes };
  for (const { mode } of conflictingModes(modes)) effective[mode] = false;
  return effective;
}

export interface CheckedTip {
  name: string;
  oid: string;
  slot: number;
}

/** A remote branch shares the colour of the local one it mirrors: `origin/x` is `x`. */
function colourName(branch: Pick<Branch, "name" | "kind">): string {
  if (branch.kind !== "remote") return branch.name;
  const cut = branch.name.indexOf("/");
  return cut < 0 ? branch.name : branch.name.slice(cut + 1);
}

/** Branches ticked in Branches, by the ids the tree ticks (`local:x`, `remote:origin/x`).
    HEAD is the main line and has its own colour. Sorted, so two tips on one commit
    always resolve the same way. */
export function checkedTips(
  branches: readonly Pick<Branch, "name" | "kind" | "oid">[],
  visible: ReadonlySet<string>,
): CheckedTip[] {
  return branches
    .filter((branch) => visible.has(`${branch.kind}:${branch.name}`))
    .map((branch) => ({ name: branch.name, oid: branch.oid, slot: branchSlot(colourName(branch)) }))
    .sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
}

/** The modes that decide which commits the walk shows; the rest only paint. The opened
    merges count only while merged branches fold, so opening one elsewhere walks nothing. */
export function graphView(modes: GraphModes, expanded: ReadonlySet<string> = new Set()): GraphView {
  const effective = effectiveModes(modes);
  return {
    firstParent: effective.firstParent,
    collapseMerged: effective.collapseMerged,
    expanded: effective.collapseMerged ? [...expanded].sort() : [],
  };
}

/** What to ask Rust to paint; `null` when there is nothing, so no call is made at all.
    The branch of a commit needs only the lanes and a fold only its count, which come
    with any paint. */
export function paintRequest(
  modes: GraphModes,
  tips: readonly CheckedTip[],
  selected: string | null = null,
): GraphPaintRequest | null {
  const painted = modes.highlightChecked ? tips.map(({ oid, slot }) => ({ oid, slot })) : [];
  const ancestryOf = modes.ancestry ? selected : null;
  const bare = modes.branchOfCommit || effectiveModes(modes).collapseMerged;
  if (painted.length === 0 && !bare && ancestryOf === null) return null;
  return ancestryOf === null ? { tips: painted } : { tips: painted, ancestryOf };
}

/** A lane chosen by clicking its line, in the row of `oid`. */
export interface LanePick {
  oid: string;
  lane: number;
}

/** The lane to bring forward: the one clicked while its row stays selected, else the
    selected commit's own. */
export function focusLane(
  modes: GraphModes,
  selected: string | null,
  selectedLane: number | null,
  pick: LanePick | null,
): number | null {
  if (!modes.branchOfCommit || selected === null) return null;
  return pick?.oid === selected ? pick.lane : selectedLane;
}
