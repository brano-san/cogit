import {
  listStashes,
  stashApply,
  stashDrop,
  stashKeepingWorktree,
  stashPush,
  type RepoId,
  type StashEntry,
} from "$lib/ipc";
import { stashRequest, type StashChoice } from "$lib/stash-modes";

class StashStore {
  entries = $state.raw<StashEntry[]>([]);

  async refresh(repo: RepoId): Promise<void> {
    this.entries = await listStashes(repo);
  }

  async push(repo: RepoId, message: string, includeUntracked: boolean): Promise<void> {
    await stashPush(repo, { message, includeUntracked, keepIndex: false });
    await this.refresh(repo);
  }

  /** One of the Stash dialog's three modes (#29). */
  async save(repo: RepoId, choice: StashChoice): Promise<void> {
    const request = stashRequest(choice);
    if (request.kind === "keepWorktree") await stashKeepingWorktree(repo, request.message);
    else {
      const { message, includeUntracked, keepIndex } = request;
      await stashPush(repo, { message, includeUntracked, keepIndex });
    }
    await this.refresh(repo);
  }

  async apply(repo: RepoId, index: number, pop: boolean): Promise<void> {
    await stashApply(repo, index, pop);
    await this.refresh(repo);
  }

  async drop(repo: RepoId, index: number): Promise<void> {
    await stashDrop(repo, index);
    await this.refresh(repo);
  }

  clear(): void {
    this.entries = [];
  }
}

export const stashes = new StashStore();
