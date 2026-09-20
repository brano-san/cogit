import { CogitError, worktreeFiles, type FileEntry, type RepoId } from "$lib/ipc";

class WorktreeStore {
  staged = $state.raw<FileEntry[]>([]);
  unstaged = $state.raw<FileEntry[]>([]);
  loading = $state(false);
  error = $state<CogitError | null>(null);

  #generation = 0;

  get total(): number {
    return this.staged.length + this.unstaged.length;
  }

  async load(repo: RepoId): Promise<void> {
    const generation = ++this.#generation;
    this.error = null;
    this.loading = true;

    try {
      const files = await worktreeFiles(repo);
      if (generation !== this.#generation) return;
      this.staged = files.staged;
      this.unstaged = files.unstaged;
    } catch (err) {
      if (generation !== this.#generation) return;
      this.staged = [];
      this.unstaged = [];
      this.error =
        err instanceof CogitError ? err : new CogitError({ kind: "internal", data: String(err) });
    } finally {
      if (generation === this.#generation) this.loading = false;
    }
  }

  clear(): void {
    this.#generation += 1;
    this.staged = [];
    this.unstaged = [];
    this.loading = false;
    this.error = null;
  }
}

export const worktree = new WorktreeStore();
