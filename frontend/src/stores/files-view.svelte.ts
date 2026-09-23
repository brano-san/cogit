import { commitView } from "$lib/file-switches";
import { DEFAULT_VIEW, mergeView, type FileView } from "$lib/file-view";

const STORAGE_KEY = "cogit.files-view.v1";
/** Apart from the working tree's, so a commit never changes the switches of the other (#3). */
const COMMIT_STORAGE_KEY = "cogit.files-view.commit.v1";

function stored(key: string): unknown {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? null : (JSON.parse(raw) as unknown);
  } catch {
    return null;
  }
}

function remember(key: string, view: FileView): void {
  try {
    localStorage.setItem(key, JSON.stringify(view));
  } catch {
    // A blocked store costs the switches on restart, nothing more.
  }
}

class FilesViewStore {
  current = $state<FileView>(mergeView(stored(STORAGE_KEY)));
  /** A commit's or a stash's list: only the switches that mean something there. */
  commit = $state<FileView>(commitView(stored(COMMIT_STORAGE_KEY)));

  set(view: FileView): void {
    this.current = view;
    remember(STORAGE_KEY, view);
  }

  setCommit(view: FileView): void {
    this.commit = commitView(view);
    remember(COMMIT_STORAGE_KEY, this.commit);
  }

  reset(): void {
    this.set({ ...DEFAULT_VIEW });
  }
}

export const filesView = new FilesViewStore();
