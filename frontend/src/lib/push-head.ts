import { events, type PushHead, type Pushed, type RepoId } from "$lib/ipc/bindings";
import { listenUntilUndone, type Emit, type Listen } from "$lib/settings-sync";
import { nothingPushed } from "$lib/toolbar";

/** Commit and Push on a detached HEAD, by the toolbar's rule (F-710): the backend's Push sends
    every branch ahead of its upstream; with none, the main window opens Push To for HEAD. */
export async function pushDetached(
  repo: RepoId,
  remote: string,
  push: (repo: RepoId, remote: string) => Promise<Pushed | null>,
  emit: Emit<PushHead> = events.cogitPushHead.emit,
): Promise<Pushed | null> {
  const pushed = await push(repo, remote);
  if (nothingPushed(pushed)) await emit({ repo });
  return pushed;
}

/** For the main window. Returns the undo, which also holds before the listener is in place. */
export function onPushHead(
  handler: (request: PushHead) => void,
  listen: Listen<PushHead> = events.cogitPushHead.listen,
): () => void {
  return listenUntilUndone(listen, handler);
}
