/**
 * The button toolbar: what each button is, and whether it can run right now.
 * Kept out of the component so the rules can be tested; the component only draws them.
 */

import { prettyKeys, shortcutOf, type Keymap } from "$lib/keymap";
import {
  DEFAULT_PREFS,
  remotesInOrder,
  type SyncOrder,
  type ToolbarPrefs,
} from "$lib/toolbar-prefs";

/** Inapplicable actions are disabled, not hidden, so buttons never move under the cursor. */
export interface ToolbarAction {
  id: string;
  label: string;
  /** Lucide path data, drawn at one size and weight. */
  icon: string;
  hint: string;
  /** The keymap command it runs, whose keys its tip shows (`shortcutOf`). */
  command?: string;
  /** A key the page keeps for itself, with no menu item to be remapped in. */
  keys?: string;
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
  applyStash: "M3 8h18M3 8l2-4h14l2 4M3 8v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V8M12 17v-6m-3 3 3-3 3 3",
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
    command: "pull",
    split: true,
  },
  {
    id: "push",
    label: "Push",
    icon: ICONS.push,
    hint: "Send your commits to the remote",
    command: "push",
    split: true,
  },
  {
    id: "sync",
    label: "Sync",
    icon: ICONS.sync,
    hint: "Pull, then push",
    command: "synchronize",
    split: true,
  },
  {
    id: "stage",
    label: "Stage",
    icon: ICONS.stage,
    hint: "Stage the selected files, or every change when none is selected",
    command: "stage",
  },
  {
    id: "unstage",
    label: "Unstage",
    icon: ICONS.unstage,
    hint: "Unstage the selected files, or the whole index when none is selected",
    command: "unstage",
  },
  {
    id: "discard",
    label: "Discard",
    icon: ICONS.discard,
    hint: "Throw away the changes in the selected files",
    keys: "CmdOrCtrl+Z",
  },
  {
    id: "stash",
    label: "Stash",
    icon: ICONS.stash,
    hint: "Put the working tree aside",
    command: "stash",
    split: true,
  },
  {
    id: "apply-stash",
    label: "Apply Stash",
    icon: ICONS.applyStash,
    hint: "Apply the newest stash (stash@{0}) and keep it in the list",
  },
  {
    id: "merge",
    label: "Merge",
    icon: ICONS.merge,
    hint: "Merge the selected commit into HEAD",
  },
  {
    id: "rebase",
    label: "Rebase",
    icon: ICONS.rebase,
    hint: "Replay HEAD on the selected commit",
    split: true,
  },
  { id: "tag", label: "Tag", icon: ICONS.tag, hint: "Tag the current commit", command: "tag" },
  { id: "undo", label: "Undo", icon: ICONS.undo, hint: "Reverse the last operation" },
];

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
  "apply-stash",
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

const SYNC_ORDERS: readonly [SyncOrder, string, string][] = [
  ["pushThenPull", "Push, then Pull", "Push first; pull only if the push worked"],
  ["pullThenPush", "Pull, then Push", "Pull first; push only if the pull worked"],
];

/** Picking an order runs it and makes it what the Sync button does from then on. */
function syncMenu(context: MenuContext): MenuEntry[] {
  return SYNC_ORDERS.map(([order, label, hint]) => ({
    kind: "radio",
    id: `sync-order:${order}`,
    label,
    hint,
    checked: context.prefs.syncOrder === order,
  }));
}

/** The keys a button's tip shows: its command's as the keymap has them, or the page's own. */
export function shortcutOfAction(action: ToolbarAction, keys: Keymap, onMac: boolean): string | undefined {
  if (action.command) return shortcutOf(action.command, keys, onMac);
  return action.keys ? prettyKeys(action.keys, onMac) : undefined;
}

/** The tooltip of a button whose action depends on a remembered choice. */
export function hintOf(action: ToolbarAction, context: MenuContext = NO_MENU_CONTEXT): string {
  if (action.id === "sync") {
    return context.prefs.syncOrder === "pushThenPull" ? "Push, then pull" : "Pull, then push";
  }
  if (action.id === "pull" && context.prefs.pullScope === "all") {
    return "Fetch every remote, then bring the current remote's commits down";
  }
  return action.hint;
}

export function menuOf(id: string, context: MenuContext = NO_MENU_CONTEXT): MenuEntry[] {
  switch (id) {
    case "pull":
      return pullMenu(context);
    case "push":
      return [
        item("push", "Push", "The current branch to its upstream"),
        item("push-to", "Push To…", "Choose the remote and the ref"),
      ];
    case "sync":
      return syncMenu(context);
    case "stash":
      return [
        item("stash-selection", "Stash Selection", "Stash the files selected in Files, after a look at the list"),
        item("quick-stash-all", "Quick Stash All", "Stash every change now, with Git's own message"),
        item("quick-stash-selection", "Quick Stash Selection", "Stash the selected files now, without asking"),
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
  branch: boolean;
  /** HEAD's branch tracks a remote branch, which Pull and Sync bring down. */
  upstream: boolean;
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
  branch: false,
  upstream: false,
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

const needBranch: Rule = (f) => needRemote(f) ?? (f.branch ? undefined : "HEAD is not on a branch");

/** Pull and Sync without an upstream, or on a detached HEAD, only ever end in git's error. */
const needUpstream: Rule = (f) =>
  needBranch(f) ?? (f.upstream ? undefined : "The branch tracks no remote branch");

const needWorkingTree: Rule = (f) =>
  needRepository(f) ??
  (f.onWorkingTree ? undefined : "Select the Working Tree in the graph first");

const anyMarked = (f: ToolbarFacts) => f.markedUnstaged.length + f.markedStaged.length > 0;

const needChanges: Rule = (f) =>
  needRepository(f) ??
  (f.unstaged.length + f.staged.length > 0 ? undefined : "The working tree is clean");

const needSelection: Rule = (f) =>
  needWorkingTree(f) ?? (anyMarked(f) ? undefined : "No file is selected in Files");

/** git stash needs a commit to stash against: before the first one it only says "You do
    not have the initial commit yet". */
const bornFirst =
  (rule: Rule): Rule =>
  (f) =>
    rule(f) ?? (f.head === null ? "Nothing is committed yet" : undefined);

const needCommit: Rule = (f) =>
  needRepository(f) ??
  (f.commit === null ? "Select a commit or a branch first" : undefined) ??
  (f.commit === f.head ? "HEAD itself is selected" : undefined);

const RULES: Record<string, Rule> = {
  pull: needUpstream,
  // A branch without an upstream is pushed with --set-upstream (R-414).
  push: needBranch,
  "push-to": needBranch,
  sync: needUpstream,
  "fetch-remote": needRemote,
  "fetch-remotes": needRemote,
  "pull-scope": needRemote,
  "delete-merged": needRemote,
  "sync-order": needRemote,
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
  stash: bornFirst(needChanges),
  "quick-stash-all": bornFirst(needChanges),
  "stash-selection": bornFirst(needSelection),
  "quick-stash-selection": bornFirst(needSelection),
  "apply-stash": (f) => needRepository(f) ?? (f.stashes > 0 ? undefined : "There are no stashes"),
  merge: (f) =>
    needCommit(f) ??
    (f.merged === undefined
      ? "Checking whether HEAD already contains it…"
      : f.merged
        ? "HEAD already contains the selected commit"
        : undefined),
  rebase: needCommit,
  "rebase-i": needCommit,
  // The selected commit, or HEAD's: an orphan branch can still tag one it selects.
  tag: (f) => needRepository(f) ?? ((f.commit ?? f.head) === null ? "There is no commit to tag yet" : undefined),
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
  action: "stage" | "unstage" | "discard" | "stash-selection",
  facts: ToolbarFacts,
): string[] {
  if (action === "stash-selection") {
    return [...new Set([...facts.markedUnstaged, ...facts.markedStaged])];
  }
  const all = action === "unstage" ? facts.staged : facts.unstaged;
  const marked = action === "unstage" ? facts.markedStaged : facts.markedUnstaged;
  if (action === "discard") return [...marked];
  return anyMarked(facts) ? [...marked] : [...all];
}

export interface BranchLike {
  name: string;
  fullName: string;
  kind: "local" | "remote";
  oid: string;
  isHead: boolean;
}

/** What Merge and Rebase hand git for a branch. Its short name, which git writes into the
    merge message as it is — unless git would read another ref first: `refs/tags/<name>`
    comes before `refs/heads/<name>`, and both before `refs/remotes/<name>`, so "Merge topic"
    merged a tag `topic` with only a warning. Then the full ref. */
export function branchRevision(
  branch: Pick<BranchLike, "name" | "fullName" | "kind">,
  branches: readonly Pick<BranchLike, "name" | "kind">[],
  tags: readonly { name: string }[],
): string {
  const shadowed =
    tags.some((tag) => tag.name === branch.name) ||
    (branch.kind === "remote" && branches.some((other) => other.kind === "local" && other.name === branch.name));
  return shadowed ? branch.fullName : branch.name;
}

/** A local branch dragged onto another in Branches, as Merge and Rebase hand it to git. */
export function localRevision(name: string, branches: readonly BranchLike[], tags: readonly { name: string }[]): string {
  const branch = branches.find((entry) => entry.kind === "local" && entry.name === name);
  return branch ? branchRevision(branch, branches, tags) : name;
}

/** What Merge and Rebase name: a branch at the selected commit reads better in the merge
    message than a hash. A local branch wins over a remote one; HEAD's own is skipped. */
export function refAt(oid: string, branches: readonly BranchLike[], tags: readonly { name: string }[]): string {
  const at = branches.filter((branch) => branch.oid === oid && !branch.isHead);
  const branch = at.find((entry) => entry.kind === "local") ?? at.find((entry) => entry.kind === "remote");
  return branch ? branchRevision(branch, branches, tags) : oid;
}
