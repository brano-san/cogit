import { lostCommits, type CommitRow, type RepoId } from "$lib/ipc";

class RecoveryStore {
  lost = $state.raw<CommitRow[]>([]);
  /** Only the newest read writes; `clear()` drops the ones in flight, which belong to the
      repository the panels are leaving. */
  #generation = 0;

  async refresh(repo: RepoId): Promise<void> {
    const generation = ++this.#generation;
    let rows: CommitRow[] = [];
    await lostCommits(repo, (chunk) => {
      if (generation !== this.#generation) return;
      rows = [...rows, ...chunk];
      this.lost = rows;
    });
    // An empty answer sends no chunk, so the old list would stay.
    if (generation === this.#generation) this.lost = rows;
  }

  clear(): void {
    this.#generation += 1;
    this.lost = [];
  }
}

export const recovery = new RecoveryStore();
