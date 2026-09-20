import type { FileEntry } from "./ipc/bindings";

/** The eight switches SmartGit puts above its file list, in its order and its words. */
export interface FileView {
  unchanged: boolean;
  untracked: boolean;
  ignored: boolean;
  assumeUnchanged: boolean;
  skipped: boolean;
  renameSources: boolean;
  directories: boolean;
  separateIndex: boolean;
}

export const DEFAULT_VIEW: FileView = {
  unchanged: false,
  untracked: true,
  ignored: false,
  assumeUnchanged: false,
  skipped: false,
  renameSources: false,
  directories: false,
  separateIndex: true,
};

export interface Toggle {
  key: keyof FileView;
  icon: string;
  title: string;
}

export const TOGGLES: readonly Toggle[] = [
  { key: "unchanged", icon: "=", title: "If selected, unchanged files will be shown" },
  {
    key: "untracked",
    icon: "?",
    title: "If selected, not yet version controlled files will be shown",
  },
  { key: "ignored", icon: "∅", title: "If selected, ignored files will be shown" },
  {
    key: "assumeUnchanged",
    icon: "≈",
    title: "If selected, files having the 'assume-unchanged' flag will be shown",
  },
  { key: "skipped", icon: "⤳", title: "If selected, skipped files will be shown" },
  {
    key: "renameSources",
    icon: "↤",
    title: "If selected, removed/missing source files of detected renames will be shown",
  },
  { key: "directories", icon: "🗀", title: "If selected, the directories will be shown" },
  {
    key: "separateIndex",
    icon: "⇅",
    title:
      "If selected and index as well as working tree changes are available, show them separately",
  },
];

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
    assumeUnchanged: view.assumeUnchanged,
    skipped: view.skipped,
  };
}

const GATED: Partial<Record<FileEntry["status"], keyof FileView>> = {
  untracked: "untracked",
  unchanged: "unchanged",
  ignored: "ignored",
  assumeUnchanged: "assumeUnchanged",
  skipped: "skipped",
};

export function visibleFiles(files: readonly FileEntry[], view: FileView): FileEntry[] {
  const shown: FileEntry[] = [];
  for (const file of files) {
    const gate = GATED[file.status];
    if (gate && !view[gate]) continue;
    shown.push(file);
    if (view.renameSources && file.oldPath) {
      shown.push({ ...file, path: file.oldPath, oldPath: null, status: "deleted" });
    }
  }
  return shown;
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
