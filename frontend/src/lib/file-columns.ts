import type { FileEntry, FileStatus } from "$lib/ipc";
import { fileName } from "$lib/files";

/** The columns of the Files table (#33), in their order on screen. */
export type ColumnKey = "name" | "type" | "extension" | "change" | "lfs" | "path";
export const COLUMN_KEYS: readonly ColumnKey[] = ["name", "type", "extension", "change", "lfs", "path"];
export const COLUMN_LABELS: Record<ColumnKey, string> = {
  name: "Name",
  type: "Type",
  extension: "Extension",
  change: "State",
  lfs: "LFS",
  path: "Path",
};

/** Every column but the name can be turned off in Customise View (#34). */
export type FileColumns = Record<Exclude<ColumnKey, "name">, boolean>;
export const DEFAULT_COLUMNS: FileColumns = { type: true, extension: false, change: true, lfs: false, path: true };

export interface FileSort {
  key: ColumnKey;
  descending: boolean;
}
export const DEFAULT_SORT: FileSort = { key: "name", descending: false };

export type FileType = "file" | "symlink" | "directory" | "repository" | "binary";
export const TYPE_LABELS: Record<FileType, string> = {
  file: "File",
  symlink: "Link",
  directory: "Folder",
  repository: "Repository",
  binary: "Binary",
};
const TYPE_ORDER: readonly FileType[] = ["file", "symlink", "directory", "repository", "binary"];

/** What needs looking at first: conflicts, then changes, then what only the switches show. */
const STATE_ORDER: readonly FileStatus[] = [
  "conflicted",
  "modified",
  "added",
  "renamed",
  "copied",
  "deleted",
  "untracked",
  "ignored",
  "assumeUnchanged",
  "skipped",
  "sparse",
  "unchanged",
];

/** By name, never by content: the status is read for thousands of files and none of them
    is opened for it. A binary file with an extension not listed here says File (R-596). */
const BINARY_EXTENSIONS = new Set([
  ...["png", "jpg", "jpeg", "gif", "bmp", "ico", "icns", "webp", "tif", "tiff", "psd", "heic", "avif"],
  ...["zip", "gz", "tgz", "bz2", "xz", "zst", "7z", "rar", "tar", "jar", "war", "apk", "aab", "nupkg"],
  ...["exe", "dll", "so", "dylib", "lib", "a", "o", "obj", "pdb", "ilk", "exp", "class", "pyc", "pyd"],
  ...["wasm", "node", "bin", "dat", "img", "iso", "dmg", "msi", "cab"],
  ...["pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp"],
  ...["mp3", "wav", "flac", "ogg", "m4a", "aac", "mp4", "mkv", "avi", "mov", "webm", "wmv"],
  ...["ttf", "otf", "woff", "woff2", "eot"],
  ...["sqlite", "db", "mdb", "pftrace", "keystore", "p12", "pfx"],
]);

/** Without the dot; `""` for a folder, a dotfile or a name with none. */
export function extensionOf(path: string): string {
  const name = fileName(path);
  const dot = name.lastIndexOf(".");
  return dot <= 0 || path.endsWith("/") ? "" : name.slice(dot + 1);
}

export function isBinaryPath(path: string): boolean {
  return BINARY_EXTENSIONS.has(extensionOf(path).toLowerCase());
}

export interface LfsState {
  lockable: boolean;
  lock: string | null;
}

/** The LFS cell: blank for a file outside Git LFS. */
export function lfsLabel(state: LfsState | undefined): string {
  if (!state) return "";
  if (state.lock !== null) return state.lock === "" ? "locked" : `locked by ${state.lock}`;
  return state.lockable ? "lockable" : "LFS";
}

/** By the entry's mode first (R-180): a submodule is a repository whatever its name. */
export function fileType(file: FileEntry): FileType {
  if (file.mode === "submodule") return "repository";
  if (file.mode === "symlink") return "symlink";
  if (file.path.endsWith("/")) return "directory";
  return isBinaryPath(file.path) ? "binary" : "file";
}

const collator = new Intl.Collator(undefined);

/** The folder a row sits in, `""` at the root; an untracked folder's own slash does not count. */
export function directoryOf(path: string): string {
  const trimmed = path.endsWith("/") ? path.slice(0, -1) : path;
  const cut = trimmed.lastIndexOf("/");
  return cut === -1 ? "" : trimmed.slice(0, cut + 1);
}

/** The order of the list. In the tree the folders stay in path order and the sort works
    inside each of them, so the rows are grouped the way they are listed. */
export function sortRows<F extends FileEntry>(
  files: readonly F[],
  sort: FileSort,
  directories: boolean,
  lfs: (path: string) => LfsState | undefined = () => undefined,
): F[] {
  const keyed = files.map((file) => {
    const name = fileName(file.path);
    let rank = 0;
    let text = "";
    if (sort.key === "extension") text = extensionOf(file.path);
    else if (sort.key === "lfs") text = lfsLabel(lfs(file.path));
    if (sort.key === "type") rank = TYPE_ORDER.indexOf(fileType(file));
    else if (sort.key === "change") rank = STATE_ORDER.indexOf(file.status);
    // The folder groupByDirectory puts the row under, so the order is the order of the rows.
    const folder = directories ? file.path.slice(0, file.path.lastIndexOf("/") + 1) : "";
    return { file, name, rank, text, folder };
  });
  const sign = sort.descending ? -1 : 1;
  keyed.sort((a, b) => {
    const folder = collator.compare(a.folder, b.folder);
    if (folder !== 0) return folder;
    let order = a.rank - b.rank || collator.compare(a.text, b.text);
    if (order === 0 && sort.key !== "path") order = collator.compare(a.name, b.name);
    if (order === 0) order = collator.compare(a.file.path, b.file.path);
    return sign * order;
  });
  return keyed.map((entry) => entry.file);
}

/** Another column sorts ascending; the same one again turns the order round. */
export function nextSort(current: FileSort, key: ColumnKey): FileSort {
  return current.key === key ? { key, descending: !current.descending } : { key, descending: false };
}

/** Why a column cannot be turned on or off here; `null` when it can. */
export function columnReason(key: ColumnKey, directories: boolean): string | null {
  if (key === "name") return "The name is always shown";
  if (key === "path" && directories) return "The folder rows show the paths while the list has directories";
  return null;
}

export function shownColumns(columns: FileColumns, directories: boolean): ColumnKey[] {
  return COLUMN_KEYS.filter((key) => key === "name" || (columns[key] && columnReason(key, directories) === null));
}

/** The Columns group of Customise View (#34): a column that cannot change here is off, with why. */
export function columnItems(
  columns: FileColumns,
  directories: boolean,
): { key: ColumnKey; label: string; checked: boolean; reason: string | null }[] {
  const shown = new Set(shownColumns(columns, directories));
  return COLUMN_KEYS.map((key) => ({
    key,
    label: COLUMN_LABELS[key],
    checked: shown.has(key),
    reason: columnReason(key, directories),
  }));
}

export function toggleColumn(columns: FileColumns, key: ColumnKey): FileColumns {
  return key === "name" ? columns : { ...columns, [key]: !columns[key] };
}

export type ColumnWidths = Record<ColumnKey, number>;

export const DEFAULT_COLUMN_WIDTHS: ColumnWidths = {
  name: 180,
  type: 65,
  extension: 70,
  change: 130,
  lfs: 110,
  path: 260,
};

export const MIN_COLUMN_WIDTH: Record<ColumnKey, number> = {
  name: 70,
  type: 45,
  extension: 45,
  change: 45,
  lfs: 45,
  path: 80,
};

/** Fixed pixel column widths so table content can be resized and overflow gracefully. */
export function gridColumns(shown: readonly ColumnKey[], widths?: Partial<ColumnWidths>): string {
  return shown
    .map((key) => {
      const width = widths?.[key] ?? DEFAULT_COLUMN_WIDTHS[key];
      return `${width}px`;
    })
    .join(" ");
}

export function mergeTable(stored: unknown): { columns: FileColumns; sort: FileSort; widths: ColumnWidths } {
  const source = typeof stored === "object" && stored !== null ? (stored as Record<string, unknown>) : {};
  const columns = { ...DEFAULT_COLUMNS };
  const keptColumns = source.columns;
  if (typeof keptColumns === "object" && keptColumns !== null) {
    for (const key of Object.keys(DEFAULT_COLUMNS) as (keyof FileColumns)[]) {
      const value = (keptColumns as Record<string, unknown>)[key];
      if (typeof value === "boolean") columns[key] = value;
    }
  }
  let sort = DEFAULT_SORT;
  const keptSort = source.sort as Record<string, unknown> | undefined;
  if (
    typeof keptSort === "object" &&
    keptSort !== null &&
    COLUMN_KEYS.includes(keptSort.key as ColumnKey) &&
    typeof keptSort.descending === "boolean"
  ) {
    sort = { key: keptSort.key as ColumnKey, descending: keptSort.descending };
  }
  const widths = { ...DEFAULT_COLUMN_WIDTHS };
  const keptWidths = source.widths as Record<string, unknown> | undefined;
  if (typeof keptWidths === "object" && keptWidths !== null) {
    for (const key of COLUMN_KEYS) {
      const val = keptWidths[key];
      if (typeof val === "number" && Number.isFinite(val) && val >= (MIN_COLUMN_WIDTH[key] ?? 30)) {
        widths[key] = Math.round(val);
      }
    }
  }
  return { columns, sort, widths };
}
