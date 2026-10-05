import { mergeTable, type ColumnWidths, type FileColumns, type FileSort, type ColumnKey, MIN_COLUMN_WIDTH } from "$lib/file-columns";
import { commitView } from "$lib/file-switches";
import { mergeView, type FileView } from "$lib/file-view";

const STORAGE_KEY = "cogit.files-view.v1";
/** Apart from the working tree's, so a commit never changes the switches of the other (#3). */
const COMMIT_STORAGE_KEY = "cogit.files-view.commit.v1";
/** One table for every list of the panel: the working tree, a commit, a stash (#33). */
const TABLE_STORAGE_KEY = "cogit.files-table.v1";

function stored(key: string): unknown {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? null : (JSON.parse(raw) as unknown);
  } catch {
    return null;
  }
}

function remember(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // A blocked store costs the switches on restart, nothing more.
  }
}

const table = mergeTable(stored(TABLE_STORAGE_KEY));

class FilesViewStore {
  current = $state<FileView>(mergeView(stored(STORAGE_KEY)));
  /** A commit's or a stash's list: only the switches that mean something there. */
  commit = $state<FileView>(commitView(stored(COMMIT_STORAGE_KEY)));
  columns = $state<FileColumns>(table.columns);
  sort = $state<FileSort>(table.sort);
  widths = $state<ColumnWidths>(table.widths);

  set(view: FileView): void {
    this.current = view;
    remember(STORAGE_KEY, view);
  }

  setCommit(view: FileView): void {
    this.commit = commitView(view);
    remember(COMMIT_STORAGE_KEY, this.commit);
  }

  /** Folders folded in Show directories, by path: a refresh keeps them as they are. */
  collapsed = $state.raw<ReadonlySet<string>>(new Set());
  /** Every folder the lists draw now, reported by them: Collapse All folds these. */
  folders = $state.raw<readonly string[]>([]);

  get anyCollapsed(): boolean {
    return this.folders.some((folder) => this.collapsed.has(folder));
  }

  toggleFolder(path: string, open?: boolean): void {
    const next = new Set(this.collapsed);
    const fold = open === undefined ? !next.has(path) : !open;
    if (fold) next.add(path);
    else next.delete(path);
    this.collapsed = next;
  }

  /** Expand All when something is folded, Collapse All otherwise. */
  toggleAll(): void {
    this.collapsed = this.anyCollapsed ? new Set() : new Set(this.folders);
  }

  setFolders(folders: readonly string[]): void {
    if (folders.length === this.folders.length && folders.every((folder, at) => folder === this.folders[at])) return;
    this.folders = folders;
  }

  setColumns(columns: FileColumns): void {
    this.columns = columns;
    this.#rememberTable();
  }

  setSort(sort: FileSort): void {
    this.sort = sort;
    this.#rememberTable();
  }

  setWidth(key: ColumnKey, width: number): void {
    const min = MIN_COLUMN_WIDTH[key] ?? 40;
    this.widths = { ...this.widths, [key]: Math.max(min, Math.round(width)) };
    this.#rememberTable();
  }

  #rememberTable(): void {
    remember(TABLE_STORAGE_KEY, { columns: this.columns, sort: this.sort, widths: this.widths });
  }
}

export const filesView = new FilesViewStore();
