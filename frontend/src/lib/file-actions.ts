import type { IndexFlag } from "./ipc/file-menus";

/** What a file menu command acts on (#40, #41). */
export interface FileScope {
  /** The right-clicked row. */
  path: string;
  /** Every file the command acts on: the ticked set when the clicked row is in it. */
  paths: string[];
  /** One per path. */
  statuses: string[];
  /** The commit of a history file; `null` on the Working Tree. */
  rev: string | null;
  /** Where the clicked file was renamed from in that commit. */
  oldPath: string | null;
}

/** The handlers the commands forward to; the shell wires them to its own actions. */
export interface FileActions {
  openFile(path: string): void;
  openVersion(path: string, rev: string): void;
  reveal(path: string): void;
  showChanges(path: string): void;
  compareWithWorkTree(path: string, rev: string): void;
  log(path: string): void;
  blame(path: string): void;
  investigate(path: string): void;
  commit(paths: string[]): void;
  stash(paths: string[]): void;
  stage(paths: string[]): void;
  unstage(paths: string[]): void;
  indexEditor(path: string): void;
  move(path: string): void;
  resolve(paths: string[], side: "ours" | "theirs"): void;
  ignore(paths: string[]): void;
  discard(paths: string[]): void;
  remove(paths: string[]): void;
  trash(paths: string[]): void;
  saveAs(path: string, rev: string): void;
  applyChange(path: string, oldPath: string | null, rev: string, reverse: boolean): void;
  copy(text: string): void;
  setFlag(paths: string[], flag: IndexFlag, on: boolean): void;
}

export interface Place {
  root: string;
  /** The platform's own, for Copy Path. */
  separator: string;
}

const FLAGGED: Record<IndexFlag, string> = {
  assumeUnchanged: "assumeUnchanged",
  skipWorktree: "skipped",
};

/** A toggle over several files sets the flag unless every one of them already has it. */
export function flagTurnsOn(statuses: readonly string[], flag: IndexFlag): boolean {
  return !(statuses.length > 0 && statuses.every((status) => status === FLAGGED[flag]));
}

function bare(path: string): string {
  return path.replace(/\/+$/, "");
}

export function fileName(path: string): string {
  const trimmed = bare(path);
  return trimmed.slice(trimmed.lastIndexOf("/") + 1);
}

/** One line per file; an absolute path is written the way the platform writes it. */
export function copiedText(
  kind: "name" | "path" | "relative",
  paths: readonly string[],
  place: Place,
): string {
  return paths
    .map((path) => {
      if (kind === "name") return fileName(path);
      if (kind === "relative") return bare(path);
      const absolute = `${bare(place.root)}/${bare(path)}`;
      return place.separator === "/" ? absolute : absolute.replaceAll("/", place.separator);
    })
    .join("\n");
}

/** Returns false for an id that is not a file menu command. */
export function runFileMenuCommand(
  id: string,
  scope: FileScope,
  actions: FileActions,
  place: Place,
): boolean {
  const { path, paths, rev } = scope;
  switch (id) {
    case "file-open":
      actions.openFile(path);
      return true;
    case "file-open-version":
      if (rev) actions.openVersion(path, rev);
      return true;
    case "file-reveal":
      actions.reveal(path);
      return true;
    case "file-changes":
      actions.showChanges(path);
      return true;
    case "file-compare-worktree":
      if (rev) actions.compareWithWorkTree(path, rev);
      return true;
    case "file-log":
      actions.log(path);
      return true;
    case "file-blame":
      actions.blame(path);
      return true;
    case "file-investigate":
      actions.investigate(path);
      return true;
    case "file-commit":
      actions.commit(paths);
      return true;
    case "file-stash":
      actions.stash(paths);
      return true;
    case "file-stage":
      actions.stage(paths);
      return true;
    case "file-unstage":
      actions.unstage(paths);
      return true;
    case "file-index-editor":
      actions.indexEditor(path);
      return true;
    case "file-move":
      actions.move(path);
      return true;
    case "file-resolve-theirs":
      actions.resolve(paths, "theirs");
      return true;
    case "file-resolve-ours":
      actions.resolve(paths, "ours");
      return true;
    case "file-ignore":
      actions.ignore(paths);
      return true;
    case "file-discard":
      actions.discard(paths);
      return true;
    case "file-remove":
      actions.remove(paths);
      return true;
    case "file-delete":
      actions.trash(paths.filter((_, at) => scope.statuses[at] !== "deleted"));
      return true;
    case "file-save-as":
      if (rev) actions.saveAs(path, rev);
      return true;
    case "file-cherry-pick":
    case "file-revert":
      if (rev) actions.applyChange(path, scope.oldPath, rev, id === "file-revert");
      return true;
    case "file-copy-name":
      actions.copy(copiedText("name", paths, place));
      return true;
    case "file-copy-path":
      actions.copy(copiedText("path", paths, place));
      return true;
    case "file-copy-relative":
      actions.copy(copiedText("relative", paths, place));
      return true;
    case "file-assume-unchanged":
      actions.setFlag(paths, "assumeUnchanged", flagTurnsOn(scope.statuses, "assumeUnchanged"));
      return true;
    case "file-skip-worktree":
      actions.setFlag(paths, "skipWorktree", flagTurnsOn(scope.statuses, "skipWorktree"));
      return true;
    default:
      return false;
  }
}
