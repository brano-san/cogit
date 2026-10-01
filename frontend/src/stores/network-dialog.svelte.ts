import * as ipc from "$lib/ipc/network-dialogs";
import type { NetworkDefaults } from "$lib/ipc/network-dialogs";
import type { NetworkApi } from "$lib/network-flow";
import { readKey, writeKey } from "$lib/settings-file";
import { network } from "$stores/network.svelte";

const KEY = "networkDialogs";

export interface PullRequest {
  kind: "pull";
  repo: number;
  /** Fetch From opens on this one. */
  remote: string;
  remotes: readonly string[];
  /** Preferences ▸ Pull: a merge may not create a merge commit. */
  ffOnly: boolean;
  defaults: NetworkDefaults;
}

export interface PushRequest {
  kind: "push";
  repo: number;
  remotes: readonly string[];
  remote: string;
  /** The branch it becomes there when the dialog opens. */
  branch: string;
  local: string;
  /** Short name of the upstream, `origin/main`. */
  upstream: string | null;
  /** Short names of every remote-tracking branch, for the branch dropdown. */
  remoteBranches: readonly string[];
  defaults: NetworkDefaults;
}

/** The Pull and Push dialogs, one open at a time; the page that opens one runs what it
    answers (it knows how to refresh the panels after). */
class NetworkDialogStore {
  open = $state.raw<PullRequest | PushRequest | null>(null);
  /** Pull's More Options block stays open or closed as the user left it. */
  moreOpen = $state(false);

  async load(): Promise<void> {
    try {
      this.moreOpen = (await readKey<{ moreOpen?: boolean }>(KEY))?.moreOpen === true;
    } catch {
      this.moreOpen = false;
    }
  }

  async setMoreOpen(open: boolean): Promise<void> {
    this.moreOpen = open;
    try {
      await writeKey(KEY, { moreOpen: open });
    } catch {
      // Applied for this session even when it cannot be saved.
    }
  }

  close(): void {
    this.open = null;
  }
}

export const networkDialog = new NetworkDialogStore();

/** Each command runs as a network operation: the footer names it and shows its progress. */
export const networkApi: NetworkApi = {
  pullWith: (repo, remote, options) =>
    network.run(repo, "Pulling", (onLine) => ipc.pullWith(repo, remote, options, onLine)),
  fetchWith: (repo, remote, options) =>
    network.run(repo, "Fetching", (onLine) => ipc.fetchWith(repo, remote, options, onLine)),
  pushWith: (repo, options) => network.run(repo, "Pushing", (onLine) => ipc.pushWith(repo, options, onLine)),
  pushNotes: (repo, remote) => network.run(repo, "Pushing notes", (onLine) => ipc.pushNotes(repo, remote, onLine)),
  mergeNotes: (repo, remote, namespace) => ipc.mergeNotes(repo, remote, namespace),
};
