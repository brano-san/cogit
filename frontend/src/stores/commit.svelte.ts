import {
  CogitError,
  commitDetails,
  commitFiles,
  type CommitDetails,
  type FileEntry,
  type RepoId,
} from "$lib/ipc";

class CommitStore {
  oid = $state<string | null>(null);
  details = $state<CommitDetails | null>(null);
  /** `$state.raw`: a commit can touch thousands of files and none of them mutate. */
  files = $state.raw<FileEntry[]>([]);
  loading = $state(false);
  error = $state<CogitError | null>(null);

  /** Clicking down the graph fires faster than the backend answers; stale replies lose. */
  #generation = 0;

  /** `onfile` is called whenever the selected commit changes, so the Files panel and the
      Diff panel cannot disagree about which file is on screen (doc/12-risks.md, R-138). */
  onchange: (() => void) | null = null;

  async select(repo: RepoId, oid: string | null): Promise<void> {
    const generation = ++this.#generation;
    if (this.oid !== oid) this.onchange?.();
    this.oid = oid;
    this.error = null;

    if (oid === null) {
      this.details = null;
      this.files = [];
      this.loading = false;
      return;
    }

    this.loading = true;
    try {
      const [details, files] = await Promise.all([
        commitDetails(repo, oid),
        commitFiles(repo, oid),
      ]);
      if (generation !== this.#generation) return;
      this.details = details;
      this.files = files;
    } catch (err) {
      if (generation !== this.#generation) return;
      this.details = null;
      this.files = [];
      this.error =
        err instanceof CogitError ? err : new CogitError({ kind: "internal", data: String(err) });
    } finally {
      if (generation === this.#generation) this.loading = false;
    }
  }

  /** The selected commit was clicked again: the Diff panel goes back to its details (#7). */
  showDetails(): void {
    this.onchange?.();
  }

  /** The graph's Working Tree row: Files and Diff go back to the working tree, whatever
      they showed — a commit, or a stash picked in References, which no commit change ends. */
  showWorkingTree(): void {
    this.onchange?.();
    this.clear();
  }

  clear(): void {
    this.#generation += 1;
    this.oid = null;
    this.details = null;
    this.files = [];
    this.loading = false;
    this.error = null;
  }
}

export const commit = new CommitStore();
