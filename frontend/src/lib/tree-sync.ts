import { emit as tauriEmit, listen as tauriListen } from "@tauri-apps/api/event";
import type { RepoId } from "$lib/ipc";
import type { Emit, Listen } from "$lib/settings-sync";

/** A compare window changed the working tree or the index (stage, unstage, discard lines):
    the main window reads its lists again, because the watcher is quiet after our own writes.
    Page-to-page like `cogit://open-module`; Rust does not declare it. */
export const TREE_CHANGED = "cogit://tree-changed";

export interface TreeChange {
  repo: RepoId;
  path: string;
}

export async function announceTreeChange(change: TreeChange, emit: Emit = tauriEmit): Promise<void> {
  await emit(TREE_CHANGED, change);
}

function isChange(payload: unknown): payload is TreeChange {
  const change = payload as Partial<TreeChange> | null;
  return typeof change?.repo === "number" && typeof change.path === "string";
}

/** For the main window. Returns the undo, which also holds before the listener is in place. */
export function onTreeChange(handler: (change: TreeChange) => void, listen: Listen = tauriListen): () => void {
  let stopped = false;
  let stop: (() => void) | null = null;
  void listen(TREE_CHANGED, (event) => {
    if (!stopped && isChange(event.payload)) handler(event.payload);
  })
    .then((unlisten) => {
      if (stopped) unlisten();
      else stop = unlisten;
    })
    .catch(() => {});
  return () => {
    stopped = true;
    stop?.();
  };
}
