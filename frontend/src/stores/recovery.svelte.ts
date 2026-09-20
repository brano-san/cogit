import { lostCommits, type CommitRow, type RepoId } from "$lib/ipc";

class RecoveryStore {
  /** Commits the reflog remembers but no ref reaches — a reset or rebase left them. */
  lost = $state.raw<CommitRow[]>([]);

  async refresh(repo: RepoId): Promise<void> {
    this.lost = await lostCommits(repo);
  }

  clear(): void {
    this.lost = [];
  }
}

export const recovery = new RecoveryStore();
