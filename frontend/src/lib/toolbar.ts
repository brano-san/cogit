/**
 * The button toolbar: what each button is, and whether it can run right now.
 *
 * Kept out of the component so the rules can be tested; the component only draws them.
 */

/** Everything the rules read, recomputed whenever the selection in Graph, Files or
    Branches changes. */
export interface ToolbarFacts {
  repository: boolean;
  remote: boolean;
  /** The Working Tree row is selected in the graph, so Files shows the working tree. */
  onWorkingTree: boolean;
  /** Paths selected in Files that have unstaged changes. */
  markedUnstaged: readonly string[];
  /** Paths selected in Files that have staged changes. */
  markedStaged: readonly string[];
  unstaged: readonly string[];
  staged: readonly string[];
  /** The commit selected in Graph or through a ref in Branches. */
  commit: string | null;
  head: string | null;
  /** Whether HEAD already contains `commit`; `undefined` while that is being asked. */
  merged: boolean | undefined;
  stashes: number;
  undo: boolean;
}

export const NO_FACTS: ToolbarFacts = {
  repository: false,
  remote: false,
  onWorkingTree: false,
  markedUnstaged: [],
  markedStaged: [],
  unstaged: [],
  staged: [],
  commit: null,
  head: null,
  merged: undefined,
  stashes: 0,
  undo: false,
};

export interface SelectionInput {
  marked: readonly string[];
  unstaged: readonly { path: string }[];
  staged: readonly { path: string }[];
}

/** The selection split by list. A path left over from before a refresh matches neither. */
export function splitMarked(input: SelectionInput): {
  markedUnstaged: string[];
  markedStaged: string[];
} {
  const unstaged = new Set(input.unstaged.map((file) => file.path));
  const staged = new Set(input.staged.map((file) => file.path));
  return {
    markedUnstaged: input.marked.filter((path) => unstaged.has(path)),
    markedStaged: input.marked.filter((path) => staged.has(path)),
  };
}

type Rule = (facts: ToolbarFacts) => string | undefined;

const needRepository: Rule = (f) => (f.repository ? undefined : "No repository is open");

const needRemote: Rule = (f) =>
  needRepository(f) ?? (f.remote ? undefined : "This repository has no remote");

const needWorkingTree: Rule = (f) =>
  needRepository(f) ??
  (f.onWorkingTree ? undefined : "Select the Working Tree in the graph first");

const anyMarked = (f: ToolbarFacts) => f.markedUnstaged.length + f.markedStaged.length > 0;

const needCommit: Rule = (f) =>
  needRepository(f) ??
  (f.commit === null ? "Select a commit or a branch first" : undefined) ??
  (f.commit === f.head ? "HEAD itself is selected" : undefined);

const RULES: Record<string, Rule> = {
  pull: needRemote,
  push: needRemote,
  sync: needRemote,
  fetch: needRemote,
  "fetch-all": needRemote,
  stage: (f) =>
    needWorkingTree(f) ??
    (anyMarked(f)
      ? f.markedUnstaged.length > 0
        ? undefined
        : "The selected files have nothing to stage"
      : f.unstaged.length > 0
        ? undefined
        : "Nothing to stage"),
  unstage: (f) =>
    needWorkingTree(f) ??
    (anyMarked(f)
      ? f.markedStaged.length > 0
        ? undefined
        : "None of the selected files is staged"
      : f.staged.length > 0
        ? undefined
        : "Nothing is staged"),
  discard: (f) =>
    needWorkingTree(f) ??
    (f.markedUnstaged.length > 0 ? undefined : "Select the changes to discard in Files"),
  stash: (f) =>
    needRepository(f) ??
    (f.unstaged.length + f.staged.length > 0 ? undefined : "The working tree is clean"),
  "stash-selection": (f) =>
    needWorkingTree(f) ?? (anyMarked(f) ? undefined : "No file is selected in Files"),
  merge: (f) =>
    needCommit(f) ??
    (f.merged === undefined
      ? "Checking whether HEAD already contains it…"
      : f.merged
        ? "HEAD already contains the selected commit"
        : undefined),
  rebase: needCommit,
  "rebase-i": needCommit,
  tag: needRepository,
  undo: (f) => needRepository(f) ?? (f.undo ? undefined : "Nothing to undo"),
};

/** Why the action cannot run, or `undefined` when it can. An unknown id is never offered. */
export function reasonOf(id: string, facts: ToolbarFacts): string | undefined {
  const rule = RULES[id];
  return rule ? rule(facts) : "Not built yet";
}

/** The files an action on the working tree applies to: the selection, or every file of
    the list when nothing is selected. */
export function targetsOf(
  action: "stage" | "unstage" | "discard",
  facts: ToolbarFacts,
): string[] {
  const all = action === "unstage" ? facts.staged : facts.unstaged;
  const marked = action === "unstage" ? facts.markedStaged : facts.markedUnstaged;
  if (action === "discard") return [...marked];
  return anyMarked(facts) ? [...marked] : [...all];
}

export interface BranchLike {
  name: string;
  kind: "local" | "remote";
  oid: string;
  isHead: boolean;
}

/** What Merge and Rebase name: a branch at the selected commit reads better in the merge
    message than a hash. A local branch wins over a remote one; HEAD's own is skipped. */
export function refAt(oid: string, branches: readonly BranchLike[]): string {
  const at = branches.filter((branch) => branch.oid === oid && !branch.isHead);
  return (
    at.find((branch) => branch.kind === "local")?.name ??
    at.find((branch) => branch.kind === "remote")?.name ??
    oid
  );
}
