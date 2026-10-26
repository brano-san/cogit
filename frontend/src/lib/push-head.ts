import { events, type PushHead, type RepoId } from "$lib/ipc/bindings";
import { listenUntilUndone, type Emit, type Listen } from "$lib/settings-sync";

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
