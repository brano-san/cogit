import { lostCommits, type CommitRow, type RepoId } from "$lib/ipc";

class RecoveryStore {
  lost = $state.raw<CommitRow[]>([]);

  async refresh(repo: RepoId): Promise<void> {
    this.lost = await lostCommits(repo);
  }

  clear(): void {
    this.lost = [];
  }
}

export const recovery = new RecoveryStore();
