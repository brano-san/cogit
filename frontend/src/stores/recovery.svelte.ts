import { lostCommits, type CommitRow, type RepoId } from "$lib/ipc";

class RecoveryStore {
  lost = $state.raw<CommitRow[]>([]);
  /** Only the newest read writes; `clear()` drops the ones in flight, which belong to the
      repository the panels are leaving. */
  #generation = 0;

  async refresh(repo: RepoId): Promise<void> {
    const generation = ++this.#generation;
    const lost = await lostCommits(repo);
    if (generation === this.#generation) this.lost = lost;
  }

  clear(): void {
    this.#generation += 1;
    this.lost = [];
  }
}

export const recovery = new RecoveryStore();
