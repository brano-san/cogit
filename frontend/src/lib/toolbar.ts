/**
 * The button toolbar: what each button is, and whether it can run right now.
 *
 * Kept out of the component so the rules can be tested; the component only draws them.
 */

import { DEFAULT_PREFS, remotesInOrder, type ToolbarPrefs } from "$lib/toolbar-prefs";

/** Inapplicable actions are disabled, not hidden, so buttons never move under the cursor. */
export interface ToolbarAction {
  id: string;
  label: string;
  /** Lucide path data, drawn at one size and weight. */
  icon: string;
  hint: string;
  shortcut?: string;
  /** A caret beside the label opens more choices; the icon runs the action itself. */
  split?: boolean;
}

export const ICONS = {
  pull: "M12 3v12m0 0 4-4m-4 4-4-4M5 21h14",
  push: "M12 21V9m0 0 4 4m-4-4-4 4M5 3h14",
  sync: "M21 12a9 9 0 0 1-9 9 9 9 0 0 1-8.5-6M3 12a9 9 0 0 1 9-9 9 9 0 0 1 8.5 6M21 4v5h-5M3 20v-5h5",
  stage: "M12 5v14m-7-7h14",
  unstage: "M5 12h14",
  discard: "M3 12a9 9 0 1 0 3-6.7L3 8m0-5v5h5",
  stash: "M3 8h18M3 8l2-4h14l2 4M3 8v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V8m-11 5h4",
  merge:
    "M7 18V9a4 4 0 0 1 4-4h5M7 6.5a2.5 2.5 0 1 0 0-.1M18.5 7.5a2.5 2.5 0 1 0 0-.1M7 20.5a2.5 2.5 0 1 0 0-.1",
  rebase:
    "M7 6.5a2.5 2.5 0 1 0 0-.1M7 20.5a2.5 2.5 0 1 0 0-.1M7 9v6M17 6.5a2.5 2.5 0 1 0 0-.1M17 9v4a4 4 0 0 1-4 4H9",
  tag: "M3 11V5a2 2 0 0 1 2-2h6l10 10-8 8L3 11Zm4-4.5a.5.5 0 1 0 0-.1",
  undo: "M3 12a9 9 0 1 1 3 6.7L3 16m0 5v-5h5",
  more: "M5 12h.01M12 12h.01M19 12h.01",
} as const;

export const ACTIONS: readonly ToolbarAction[] = [
  {
    id: "pull",
    label: "Pull",
    icon: ICONS.pull,
    hint: "Bring the remote's commits down",
    shortcut: "Ctrl+Shift+U",
    split: true,
  },
  {
    id: "push",
    label: "Push",
    icon: ICONS.push,
    hint: "Send your commits to the remote",
    shortcut: "Ctrl+Shift+O",
  },
  {
    id: "sync",
    label: "Sync",
    icon: ICONS.sync,
    hint: "Fetch every remote",
    shortcut: "Ctrl+Shift+S",
  },
  {
    id: "stage",
    label: "Stage",
    icon: ICONS.stage,
    hint: "Stage the selected files, or every change when none is selected",
    shortcut: "Ctrl+T",
  },
  {
    id: "unstage",
    label: "Unstage",
    icon: ICONS.unstage,
    hint: "Unstage the selected files, or the whole index when none is selected",
    shortcut: "Ctrl+Shift+T",
  },
  {
    id: "discard",
    label: "Discard",
    icon: ICONS.discard,
    hint: "Throw away the changes in the selected files",
    shortcut: "Ctrl+Z",
  },
  {
    id: "stash",
    label: "Stash",
    icon: ICONS.stash,
    hint: "Put the working tree aside",
    shortcut: "Ctrl+S",
    split: true,
  },
  {
    id: "merge",
    label: "Merge",
    icon: ICONS.merge,
    hint: "Merge the selected commit into HEAD",
    shortcut: "Ctrl+M",
  },
  {
    id: "rebase",
    label: "Rebase",
    icon: ICONS.rebase,
    hint: "Replay HEAD on the selected commit",
    shortcut: "Ctrl+R",
    split: true,
  },
  { id: "tag", label: "Tag", icon: ICONS.tag, hint: "Tag the current commit", shortcut: "Shift+F7" },
  { id: "undo", label: "Undo", icon: ICONS.undo, hint: "Reverse the last operation" },
];

/** Where one group of buttons ends and the next begins. */
export const SEPARATOR = "|";

export const DEFAULT_LAYOUT: readonly string[] = [
  "pull",
  "push",
  "sync",
  SEPARATOR,
  "stage",
  "unstage",
  "discard",
  SEPARATOR,
  "stash",
  "merge",
  "rebase",
  "tag",
  SEPARATOR,
  "undo",
];

export function actionOf(id: string): ToolbarAction | undefined {
  return ACTIONS.find((action) => action.id === id);
}

/** The buttons in their groups. Unknown ids and empty groups drop out. */
export function groupsOf(layout: readonly string[]): ToolbarAction[][] {
  const groups: ToolbarAction[][] = [[]];
  for (const id of layout) {
    if (id === SEPARATOR) {
      groups.push([]);
      continue;
    }
    const action = actionOf(id);
    if (action) groups[groups.length - 1]?.push(action);
  }
  return groups.filter((group) => group.length > 0);
}

export type MenuEntry =
  | { kind: "item"; id: string; label: string; hint: string }
  | { kind: "radio" | "check"; id: string; label: string; hint: string; checked: boolean }
  | { kind: "separator" };

/** What the dropdowns are built from besides the fixed entries. */
export interface MenuContext {
  remotes: readonly string[];
  /** The remote Pull uses; see `currentRemote`. */
  current: string | null;
  prefs: ToolbarPrefs;
}

export const NO_MENU_CONTEXT: MenuContext = { remotes: [], current: null, prefs: DEFAULT_PREFS };

const item = (id: string, label: string, hint: string): MenuEntry => ({
  kind: "item",
  id,
  label,
  hint,
});

function pullMenu(context: MenuContext): MenuEntry[] {
  const { prefs } = context;
  const fetches = remotesInOrder(context.remotes, context.current).map((remote) =>
    item(
      `fetch-remote:${remote}`,
      remote === context.current ? `Fetch '${remote}' (current)` : `Fetch '${remote}'`,
      `Update the refs of ${remote}; change nothing here`,
    ),
  );
  return [
    item(
      "pull",
      "Pull",
      prefs.pullScope === "all"
        ? "Fetch every remote, then merge from the current one"
        : "Fetch the current remote, then merge",
    ),
    { kind: "separator" },
    ...fetches,
    item("fetch-remotes", "Fetch All", "Update the refs of every remote of this repository"),
    { kind: "separator" },
    {
      kind: "radio",
      id: "pull-scope:current",
      label: "Pull Uses the Current Remote",
      hint: "The Pull button fetches only the remote the branch tracks",
      checked: prefs.pullScope === "current",
    },
    {
      kind: "radio",
      id: "pull-scope:all",
      label: "Pull Uses All Remotes",
      hint: "The Pull button fetches every remote before it merges",
      checked: prefs.pullScope === "all",
    },
    { kind: "separator" },
    {
      kind: "check",
      id: "delete-merged",
      label: "Delete Merged Branches after Pull",
      hint: "Delete local branches merged into HEAD whose upstream the remote deleted",
      checked: prefs.deleteMergedAfterPull,
    },
  ];
}

/** The dropdown of a split button. */
export function menuOf(id: string, context: MenuContext = NO_MENU_CONTEXT): MenuEntry[] {
  switch (id) {
    case "pull":
      return pullMenu(context);
    case "stash":
      return [
        item("stash", "Stash All", "Everything in the working tree"),
        item("stash-selection", "Stash Selection", "Only the selected files"),
      ];
    case "rebase":
      return [
        item("rebase", "Rebase", "Replay HEAD on the selected commit"),
        item("rebase-i", "Interactive Rebase…", "Edit the list of commits first"),
      ];
    default:
      return [];
  }
}

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
  "fetch-remote": needRemote,
  "fetch-remotes": needRemote,
  "pull-scope": needRemote,
  "delete-merged": needRemote,
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

/** Why the action cannot run, or `undefined` when it can. An unknown id is never offered;
    `fetch-remote:origin` follows the rule of `fetch-remote`. */
export function reasonOf(id: string, facts: ToolbarFacts): string | undefined {
  const rule = RULES[id] ?? RULES[id.split(":")[0] ?? ""];
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
