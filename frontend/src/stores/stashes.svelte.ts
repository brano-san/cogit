import { listStashes, stashApply, stashDrop, stashPush, type RepoId, type StashEntry } from "$lib/ipc";

class StashStore {
  entries = $state.raw<StashEntry[]>([]);

  async refresh(repo: RepoId): Promise<void> {
    this.entries = await listStashes(repo);
  }

  async push(repo: RepoId, message: string, includeUntracked: boolean): Promise<void> {
    await stashPush(repo, { message, includeUntracked, keepIndex: false });
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
