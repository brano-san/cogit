import { events, type PushHead, type RepoId } from "$lib/ipc/bindings";
import { listenUntilUndone, type Emit, type Listen } from "$lib/settings-sync";

/** Commit and Push: the main window pushes as the toolbar's Push does (F-710), so the
    commit window closes at once and the graph refreshes after the push. */
export async function requestPush(repo: RepoId, emit: Emit<PushHead> = events.cogitPushHead.emit): Promise<void> {
  await emit({ repo });
}

/** For the main window. Returns the undo, which also holds before the listener is in place. */
export function onPushHead(
  handler: (request: PushHead) => void,
  listen: Listen<PushHead> = events.cogitPushHead.listen,
): () => void {
  return listenUntilUndone(listen, handler);
}
