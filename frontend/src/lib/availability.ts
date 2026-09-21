/**
 * One place that decides whether a command can run, and says why not.
 *
 * The toolbar, the command palette and the native menu all ask this, so a button and
 * its menu entry can never disagree about whether an action is offered (issue 2).
 */

/** What a command needs before it makes sense. Absent means "no requirement". */
export interface Requires {
  repository?: boolean;
  /** A remote to talk to; implies a repository. */
  remote?: boolean;
  /** Files ticked in the Files panel. */
  selection?: boolean;
  /** Something in the working tree to act on. */
  changes?: boolean;
  /** Something in the index. */
  staged?: boolean;
  /** A commit picked in the graph. */
  commit?: boolean;
  /** HEAD on a branch rather than detached. */
  branch?: boolean;
  /** Something the safety journal can reverse. */
  undo?: boolean;
  /** A file showing in the Diff panel. */
  file?: boolean;
}

/** The state the rules read. One object, built once per render. */
export interface Context {
  repository: boolean;
  remote: boolean;
  selection: boolean;
  changes: boolean;
  staged: boolean;
  commit: boolean;
  branch: boolean;
  undo: boolean;
  file: boolean;
}

export const NOTHING: Context = {
  repository: false,
  remote: false,
  selection: false,
  changes: false,
  staged: false,
  commit: false,
  branch: false,
  undo: false,
  file: false,
};

/** Checked in this order, so the most fundamental reason is the one shown. */
const RULES: readonly (readonly [keyof Requires, keyof Context, string])[] = [
  ["repository", "repository", "No repository is open"],
  ["remote", "remote", "This repository has no remote"],
  ["branch", "branch", "HEAD is not on a branch"],
  ["commit", "commit", "Select a commit first"],
  ["selection", "selection", "No file is ticked"],
  ["staged", "staged", "Nothing is staged"],
  ["changes", "changes", "The working tree is clean"],
  ["file", "file", "No file is open in the Diff panel"],
  ["undo", "undo", "Nothing to undo"],
];

/**
 * Why `requires` cannot be satisfied, or `undefined` when it can.
 *
 * Anything that needs a remote needs a repository first, whether or not it said so;
 * the same for the other per-repository conditions. Otherwise a closed window would
 * explain that the repository has no remote.
 */
export function reasonFor(requires: Requires, context: Context): string | undefined {
  const needsRepository =
    requires.repository ||
    requires.remote ||
    requires.branch ||
    requires.commit ||
    requires.selection ||
    requires.staged ||
    requires.changes;

  if (needsRepository && !context.repository) return "No repository is open";

  for (const [flag, field, reason] of RULES) {
    if (requires[flag] && !context[field]) return reason;
  }
  return undefined;
}

/** The reason for every command in one pass, keyed by id, for a whole toolbar or menu. */
export function reasons(
  commands: Readonly<Record<string, Requires>>,
  context: Context,
): Record<string, string | undefined> {
  const out: Record<string, string | undefined> = {};
  for (const [id, requires] of Object.entries(commands)) {
    out[id] = reasonFor(requires, context);
  }
  return out;
}
