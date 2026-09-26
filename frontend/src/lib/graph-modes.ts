import type { GraphColoring } from "$lib/graph-coloring";
import { branchSlot } from "$lib/graph-style";
import type { Branch, CommitQuery, GraphPaintRequest, GraphView } from "$lib/ipc";
import { isEmptyQuery } from "$lib/query";

/** The graph settings this module reads, by the parameter names of the settings contract
    (`graphHighlightChecked` → `highlightChecked`, …). */
export interface GraphModes {
  highlightChecked: boolean;
  /** `--first-parent`: one line per ticked ref, merged branches left out. */
  firstParent: boolean;
  /** `branch`: a click on a commit or its line brings its branch forward; `mergeable`: what
      merging the selected commit would bring stands out; `varying`: a color per lane. */
  coloring: GraphColoring;
  /** Everything but the chosen commit's ancestors and descendants is dimmed. */
  ancestry: boolean;
  /** A branch merged in is one row at its merge, opened with a button there. */
  collapseMerged: boolean;
  /** A filtered list is drawn with lines between its matches instead of flat. */
  filteredGraph: boolean;
}

/** The modes that are switches; the coloring is a choice of four. */
export type GraphMode = Exclude<keyof GraphModes, "coloring">;

/** As the graph looked before the settings existed, plus colour for ticked branches. */
export const GRAPH_MODE_DEFAULTS: Readonly<GraphModes> = {
  highlightChecked: true,
  firstParent: false,
  coloring: "default",
  ancestry: false,
  collapseMerged: false,
  filteredGraph: false,
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

/** Switches that do nothing under a coloring: both dim, and Mergeable decides what. */
export const COLORING_CONFLICTS: readonly { mode: GraphMode; coloring: GraphColoring; reason: string }[] = [
  {
    mode: "ancestry",
    coloring: "mergeable",
    reason: "Mergeable Coloring already dims all but what a merge would bring.",
  },
];

/** The modes that are inactive under `modes`, and why: Preferences greys them out. */
export function conflictingModes(modes: GraphModes): { mode: GraphMode; reason: string }[] {
  const byMode = MODE_CONFLICTS.filter((conflict) => modes[conflict.by]);
  const byColoring = COLORING_CONFLICTS.filter((conflict) => modes.coloring === conflict.coloring);
  return [...byMode, ...byColoring].map(({ mode, reason }) => ({ mode, reason }));
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
    ...(effective.filteredGraph ? { filteredGraph: true } : {}),
  };
}

/** The view a load sends: the lines between matches only with a filter, so switching them
    on leaves the whole history's walk, and the graph kept in Rust for it, as it was. */
export function walkedView(view: GraphView, query: CommitQuery): GraphView {
  if (!isEmptyQuery(query) || view.filteredGraph === undefined) return view;
  const walk = { ...view };
  delete walk.filteredGraph;
  return walk;
}

/** Whether a new view needs a new walk: a filtered list takes only the lines between its
    matches from it, the whole history everything but those (R-51, R-575). */
export function viewReloads(before: GraphView, after: GraphView, filtered: boolean): boolean {
  const lines = (view: GraphView) => view.filteredGraph ?? false;
  if (filtered) return lines(before) !== lines(after);
  const walked = (view: GraphView) => JSON.stringify({ ...view, filteredGraph: undefined });
  return walked(before) !== walked(after);
}

/** What to ask Rust to paint; `null` when there is nothing, so no call is made at all.
    The branch of a commit needs only the lanes and a fold only its count, which come
    with any paint. */
export function paintRequest(
  modes: GraphModes,
  tips: readonly CheckedTip[],
  selected: string | null = null,
): GraphPaintRequest | null {
  const effective = effectiveModes(modes);
  const painted = modes.highlightChecked ? tips.map(({ oid, slot }) => ({ oid, slot })) : [];
  const ancestryOf = effective.ancestry ? selected : null;
  const mergeableOf = modes.coloring === "mergeable" ? selected : null;
  const bare = modes.coloring === "branch" || effective.collapseMerged;
  if (painted.length === 0 && !bare && ancestryOf === null && mergeableOf === null) return null;
  return {
    tips: painted,
    ...(ancestryOf === null ? {} : { ancestryOf }),
    ...(mergeableOf === null ? {} : { mergeableOf }),
  };
}

/** A lane chosen by clicking its line, in the row of `oid`. Lanes are numbered per walk,
    so the number means nothing in another one. */
export interface LanePick {
  oid: string;
  lane: number;
  walk: string;
}

/** The lane to bring forward: the one clicked while its row stays selected in the same
    walk, else the selected commit's own. */
export function focusLane(
  modes: GraphModes,
  selected: string | null,
  selectedLane: number | null,
  pick: LanePick | null,
  walk: string,
): number | null {
  if (modes.coloring !== "branch" || selected === null) return null;
  return pick?.oid === selected && pick.walk === walk ? pick.lane : selectedLane;
}
