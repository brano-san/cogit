import { mergeTable, type FileColumns, type FileSort } from "$lib/file-columns";
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

  set(view: FileView): void {
    this.current = view;
    remember(STORAGE_KEY, view);
  }

  setCommit(view: FileView): void {
    this.commit = commitView(view);
    remember(COMMIT_STORAGE_KEY, this.commit);
  }

  setColumns(columns: FileColumns): void {
    this.columns = columns;
    this.#rememberTable();
  }

  setSort(sort: FileSort): void {
    this.sort = sort;
    this.#rememberTable();
  }

  #rememberTable(): void {
    remember(TABLE_STORAGE_KEY, { columns: this.columns, sort: this.sort });
  }
}

export const filesView = new FilesViewStore();
