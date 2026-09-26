import { commitTreeFiles, toCogitError, type CogitError, type RepoId } from "$lib/ipc";

/** The whole tree of the selected commit, read only while its Unchanged switch is on (#3). */
class CommitTreeStore {
  oid = $state<string | null>(null);
  paths = $state.raw<string[] | null>(null);
  /** Reported by the Files panel; the changed files are listed all the same. */
  error = $state<CogitError | null>(null);

  #generation = 0;
  /** Not state: the panel's effect reads what `load` reads, and must not rerun on it. */
  #failed = false;

  async load(repo: RepoId, oid: string): Promise<void> {
    if (this.oid === oid && !this.#failed) return;
    const generation = ++this.#generation;
    this.oid = oid;
    this.paths = null;
    this.#failed = false;
    this.error = null;
    try {
      const paths = await commitTreeFiles(repo, oid);
      if (generation === this.#generation) this.paths = paths;
    } catch (err) {
      if (generation !== this.#generation) return;
      // Asked again the next time the switch goes on or the commit is picked again.
      this.#failed = true;
      this.paths = [];
      this.error = toCogitError(err);
    }
  }

  clear(): void {
    this.#generation += 1;
    this.#failed = false;
    this.oid = null;
    this.paths = null;
    this.error = null;
  }
}

export const commitTree = new CommitTreeStore();
