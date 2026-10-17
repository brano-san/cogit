import { type RepoId, unpushedCommits } from "$lib/ipc";

/** Commits no remote-tracking branch reaches: the graph draws them as hollow rings. Asked
    for with every load of the graph, which every commit, fetch, push, reset and rebase
    already brings; never on a redraw. */
class UnpushedStore {
  #oids = $state.raw<ReadonlySet<string>>(new Set());
  #asks = 0;

  has(oid: string): boolean {
    return this.#oids.has(oid);
  }

  /** A failed question counts every commit as pushed, the safe reading (crates/CLAUDE.md). */
  async refresh(repo: RepoId): Promise<void> {
    const ask = ++this.#asks;
    const oids = await unpushedCommits(repo).catch(() => [] as string[]);
    if (ask === this.#asks) this.#oids = new Set(oids);
  }

  clear(): void {
    this.#asks += 1;
    this.#oids = new Set();
  }
}

export const unpushed = new UnpushedStore();
