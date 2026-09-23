import { commitTreeFiles, type RepoId } from "$lib/ipc";

/** The whole tree of the selected commit, read only while its Unchanged switch is on (#3). */
class CommitTreeStore {
  oid = $state<string | null>(null);
  paths = $state.raw<string[] | null>(null);

  #generation = 0;

  async load(repo: RepoId, oid: string): Promise<void> {
    if (this.oid === oid) return;
    const generation = ++this.#generation;
    this.oid = oid;
    this.paths = null;
    try {
      const paths = await commitTreeFiles(repo, oid);
      if (generation === this.#generation) this.paths = paths;
    } catch {
      // The changed files are still listed; the unchanged ones are an extra.
      if (generation === this.#generation) this.paths = [];
    }
  }

  clear(): void {
    this.#generation += 1;
    this.oid = null;
    this.paths = null;
  }
}

export const commitTree = new CommitTreeStore();
