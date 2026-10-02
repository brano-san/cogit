import { events } from "$lib/ipc/bindings";
import type { TreeChanged } from "$lib/ipc";
import { listenUntilUndone, type Emit, type Listen } from "$lib/settings-sync";

/** A compare window changed the working tree or the index (stage, unstage, discard lines):
    the main window reads its lists again, because the watcher is quiet after our own writes. */
export async function announceTreeChange(
  change: TreeChanged,
  emit: Emit<TreeChanged> = events.cogitTreeChanged.emit,
): Promise<void> {
  await emit(change);
}

/** For the main window. Returns the undo, which also holds before the listener is in place. */
export function onTreeChange(
  handler: (change: TreeChanged) => void,
  listen: Listen<TreeChanged> = events.cogitTreeChanged.listen,
): () => void {
  return listenUntilUndone(listen, handler);
}
