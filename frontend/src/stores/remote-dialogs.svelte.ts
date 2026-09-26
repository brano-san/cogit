import type { RemoteInfo } from "$lib/ipc/remotes";

/** The dialogs of a remote's menu in Branches (`RefGroupActions`). They name a remote of
    the repository on screen, so leaving it closes them (`lib/leaving.ts`). */
class RemoteDialogs {
  properties = $state.raw<RemoteInfo | null>(null);

  close(): void {
    this.properties = null;
  }
}

export const remoteDialogs = new RemoteDialogs();
