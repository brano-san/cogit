import { DEFAULT_VIEW, mergeView, type FileView } from "$lib/file-view";

const STORAGE_KEY = "cogit.files-view.v1";

function stored(): unknown {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw === null ? null : (JSON.parse(raw) as unknown);
  } catch {
    return null;
  }
}

class FilesViewStore {
  current = $state<FileView>(mergeView(stored()));

  set(view: FileView): void {
    this.current = view;
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(view));
    } catch {
      // A blocked store costs the switches on restart, nothing more.
    }
  }

  reset(): void {
    this.set({ ...DEFAULT_VIEW });
  }
}

export const filesView = new FilesViewStore();
