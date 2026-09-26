import type { FileEntry } from "./ipc/bindings";

/** The switches SmartGit puts above its file list, in its order and its words. */
export interface FileView {
  unchanged: boolean;
  untracked: boolean;
  ignored: boolean;
  /** Skip-worktree and assume-unchanged alike: Git does not look at either file (R-420). */
  skipped: boolean;
  renameSources: boolean;
  directories: boolean;
  separateIndex: boolean;
  /** SmartGit lists these two beside the others; Cogit had them always on (issue 11). */
  modified: boolean;
  missing: boolean;
  /** The filter field reads the text as a pattern rather than a substring. */
  regex: boolean;
  /** Look inside the files, not only at their names (issue 10.3). */
  contents: boolean;
}

export const DEFAULT_VIEW: FileView = {
  unchanged: false,
  untracked: true,
  ignored: false,
  skipped: false,
  renameSources: false,
  directories: false,
  separateIndex: true,
  modified: true,
  missing: true,
  regex: false,
  contents: false,
};

export interface BackendView {
  unchanged: boolean;
  ignored: boolean;
  assumeUnchanged: boolean;
  skipped: boolean;
}

/** Only these four cost the backend extra reading; the rest filter what is already here. */
export function backendView(view: FileView): BackendView {
  return {
    unchanged: view.unchanged,
    ignored: view.ignored,
    assumeUnchanged: view.skipped,
    skipped: view.skipped,
  };
}

const GATED: Partial<Record<FileEntry["status"], keyof FileView>> = {
  untracked: "untracked",
  unchanged: "unchanged",
  ignored: "ignored",
  assumeUnchanged: "skipped",
  skipped: "skipped",
  modified: "modified",
  deleted: "missing",
};

export function visibleFiles(files: readonly FileEntry[], view: FileView): FileEntry[] {
  const shown: FileEntry[] = [];
  for (const file of files) {
    const gate = GATED[file.status];
    if (gate && !view[gate]) continue;
    shown.push(file);
    if (view.renameSources && file.status === "renamed" && file.oldPath) {
      shown.push({ ...file, path: file.oldPath, oldPath: null, status: "deleted" });
    }
  }
  return shown;
}

/** Rows that came and are not shown. The sources `renameSources` adds are not rows that
    came, so they cannot make up for hidden ones. */
export function hiddenCount(
  files: readonly FileEntry[],
  view: FileView,
  keep: (file: FileEntry) => boolean,
): number {
  let hidden = 0;
  for (const file of files) {
    const gate = GATED[file.status];
    if ((gate && !view[gate]) || !keep(file)) hidden += 1;
  }
  return hidden;
}

/** The switches keeping rows that came out of sight. Unchanged and ignored rows only come
    while their switch is on, so those two are never among them. */
export function hidingSwitches(files: readonly FileEntry[], view: FileView): (keyof FileView)[] {
  const keys = new Set<keyof FileView>();
  for (const file of files) {
    const gate = GATED[file.status];
    if (gate && !view[gate]) keys.add(gate);
  }
  return [...keys];
}

export type ViewRow =
  | { kind: "dir"; path: string; count: number }
  | { kind: "file"; file: FileEntry };

/** The repository root is `""`, not `"/"`: a leading slash reads like an absolute path. */
export function groupByDirectory(files: readonly FileEntry[], on: boolean): ViewRow[] {
  if (!on) return files.map((file) => ({ kind: "file", file }));

  const order: string[] = [];
  const groups = new Map<string, FileEntry[]>();
  for (const file of files) {
    const cut = file.path.lastIndexOf("/");
    const directory = cut === -1 ? "" : file.path.slice(0, cut + 1);
    const bucket = groups.get(directory);
    if (bucket) bucket.push(file);
    else {
      groups.set(directory, [file]);
      order.push(directory);
    }
  }

  const rows: ViewRow[] = [];
  for (const directory of order) {
    const bucket = groups.get(directory) ?? [];
    rows.push({ kind: "dir", path: directory, count: bucket.length });
    for (const file of bucket) rows.push({ kind: "file", file });
  }
  return rows;
}

export function mergeView(stored: unknown): FileView {
  const merged = { ...DEFAULT_VIEW };
  if (typeof stored !== "object" || stored === null) return merged;

  const source = stored as Record<string, unknown>;
  for (const key of Object.keys(DEFAULT_VIEW) as (keyof FileView)[]) {
    if (typeof source[key] === "boolean") merged[key] = source[key];
  }
  return merged;
}

/** An empty Staged pane is a splitter and a heading around nothing. */
export function shownSections<S extends { files: readonly unknown[]; hideWhenEmpty?: boolean }>(
  sections: readonly S[],
): S[] {
  return sections.filter((section) => section.files.length > 0 || !section.hideWhenEmpty);
}

/** Headings belong to a list of several sections, even while one of them is hidden: they
    carry Stage all and Unstage all. Separate panes stay separate with one of them hidden. */
export function paneLayout(sections: number, visible: number, separate: boolean) {
  const titled = sections > 1;
  return { apart: separate && titled && visible > 0, titled };
}
