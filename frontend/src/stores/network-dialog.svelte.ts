import * as ipc from "$lib/ipc/network-dialogs";
import type { NetworkDefaults } from "$lib/ipc/network-dialogs";
import type { PushBranch } from "$lib/network-dialogs";
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
  /** HEAD is detached: only Fetch Only runs (F-710). */
  detached?: boolean;
}

export interface PushRequest {
  kind: "push";
  repo: number;
  remotes: readonly string[];
  remote: string;
  /** The branch it becomes there when the dialog opens. */
  branch: string;
  local: string;
  /** Every local branch, for Local branch; `local` is one of them. */
  locals: readonly PushBranch[];
  /** Short names of every remote-tracking branch, for the branch dropdown. */
  remoteBranches: readonly string[];
  defaults: NetworkDefaults;
}

/** The Pull and Push dialogs, one open at a time; the page that opens one runs what it
    answers (it knows how to refresh the panels after). */
class NetworkDialogStore {
  open = $state.raw<PullRequest | PushRequest | null>(null);
  /** Pull's More Options and Push's Advanced stay open or closed as the user left them. */
  moreOpen = $state(false);
  advancedOpen = $state(false);

  async load(): Promise<void> {
    try {
      const saved = await readKey<{ moreOpen?: boolean; advancedOpen?: boolean }>(KEY);
      this.moreOpen = saved?.moreOpen === true;
      this.advancedOpen = saved?.advancedOpen === true;
    } catch {
      this.moreOpen = false;
      this.advancedOpen = false;
    }
  }

  async setMoreOpen(open: boolean): Promise<void> {
    this.moreOpen = open;
    await this.save();
  }

  async setAdvancedOpen(open: boolean): Promise<void> {
    this.advancedOpen = open;
    await this.save();
  }

  private async save(): Promise<void> {
    try {
      await writeKey(KEY, { moreOpen: this.moreOpen, advancedOpen: this.advancedOpen });
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
    network.run(repo, "Pulling", (onLine, credential) => ipc.pullWith(repo, remote, options, onLine, credential)),
  fetchWith: (repo, remote, options) =>
    network.run(repo, "Fetching", (onLine, credential) => ipc.fetchWith(repo, remote, options, onLine, credential)),
  pushWith: (repo, options) => network.run(repo, "Pushing", (onLine, credential) => ipc.pushWith(repo, options, onLine, credential)),
  pushNotes: (repo, remote) => network.run(repo, "Pushing notes", (onLine, credential) => ipc.pushNotes(repo, remote, onLine, credential)),
  mergeNotes: (repo, remote, namespace) => ipc.mergeNotes(repo, remote, namespace),
};
