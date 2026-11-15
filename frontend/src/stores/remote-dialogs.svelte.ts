import type { RemoteInfo } from "$lib/ipc/remotes";

/** The dialogs of a remote's menu in Branches (`RefGroupActions`). They name a remote of
    the repository on screen, so leaving it closes them (`lib/leaving.ts`). */
class RemoteDialogs {
  properties = $state.raw<RemoteInfo | null>(null);
  /** Add Remote… (F-941), from Branches or Remote ▸ Add Remote…. */
  adding = $state(false);

  close(): void {
    this.properties = null;
    this.adding = false;
  }
}

export const remoteDialogs = new RemoteDialogs();
