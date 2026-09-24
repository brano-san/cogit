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
}

/** As the graph looked before the settings existed, plus colour for ticked branches. */
export const GRAPH_MODE_DEFAULTS: Readonly<GraphModes> = {
  highlightChecked: true,
  firstParent: false,
  branchOfCommit: false,
};

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

/** The modes that decide which commits the walk shows; the rest only paint. */
export function graphView(modes: GraphModes): GraphView {
  return { firstParent: modes.firstParent };
}

/** What to ask Rust to paint; `null` when there is nothing, so no call is made at all.
    The branch of a commit needs only the lanes, which come with any paint. */
export function paintRequest(modes: GraphModes, tips: readonly CheckedTip[]): GraphPaintRequest | null {
  const painted = modes.highlightChecked ? tips.map(({ oid, slot }) => ({ oid, slot })) : [];
  if (painted.length === 0 && !modes.branchOfCommit) return null;
  return { tips: painted };
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
