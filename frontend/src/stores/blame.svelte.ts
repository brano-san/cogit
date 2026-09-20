import { blameFile, CogitError, type BlameLine, type RepoId } from "$lib/ipc";

class BlameStore {
  path = $state<string | null>(null);
  lines = $state.raw<BlameLine[]>([]);
  loading = $state(false);
  error = $state<CogitError | null>(null);

  #generation = 0;

  async show(repo: RepoId, path: string, rev: string): Promise<void> {
    const generation = ++this.#generation;
    this.path = path;
    this.error = null;
    this.loading = true;
    try {
      const lines = await blameFile(repo, path, rev);
      if (generation !== this.#generation) return;
      this.lines = lines;
    } catch (err) {
      if (generation !== this.#generation) return;
      this.lines = [];
      this.path = null;
      this.error =
        err instanceof CogitError ? err : new CogitError({ kind: "internal", data: String(err) });
    } finally {
      if (generation === this.#generation) this.loading = false;
    }
  }

  clear(): void {
    this.#generation += 1;
    this.path = null;
    this.lines = [];
    this.loading = false;
    this.error = null;
  }
}

export const blame = new BlameStore();
