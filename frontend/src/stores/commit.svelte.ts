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

  async select(repo: RepoId, oid: string | null): Promise<void> {
    const generation = ++this.#generation;
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
