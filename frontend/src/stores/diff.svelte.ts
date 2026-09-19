import { CogitError, diffFile, type FileDiff, type RepoId } from "$lib/ipc";

class DiffStore {
  path = $state<string | null>(null);
  /** `$state.raw`: a diff is replaced wholesale and can hold tens of thousands of rows. */
  diff = $state.raw<FileDiff | null>(null);
  loading = $state(false);
  error = $state<CogitError | null>(null);

  /** Clicking down the file list outruns the backend; stale diffs lose. */
  #generation = 0;

  async load(repo: RepoId, oid: string, path: string): Promise<void> {
    const generation = ++this.#generation;
    this.path = path;
    this.error = null;
    this.loading = true;

    try {
      const result = await diffFile(repo, { kind: "commitVsParent", oid }, path);
      if (generation !== this.#generation) return;
      this.diff = result;
    } catch (err) {
      if (generation !== this.#generation) return;
      this.diff = null;
      this.error =
        err instanceof CogitError ? err : new CogitError({ kind: "internal", data: String(err) });
    } finally {
      if (generation === this.#generation) this.loading = false;
    }
  }

  clear(): void {
    this.#generation += 1;
    this.path = null;
    this.diff = null;
    this.loading = false;
    this.error = null;
  }
}

export const diff = new DiffStore();
