import { backendView } from "$lib/file-view";
import { filesView } from "$stores/files-view.svelte";
import {
  CogitError,
  createCommit,
  discardPaths,
  stagePaths,
  unstagePaths,
  worktreeFiles,
  type FileEntry,
  type RepoId,
} from "$lib/ipc";

class WorktreeStore {
  staged = $state.raw<FileEntry[]>([]);
  unstaged = $state.raw<FileEntry[]>([]);
  loading = $state(false);
  error = $state<CogitError | null>(null);

  #generation = 0;
  /** Bumped by `clear()`: a write that finishes after it reads nothing back, since the
      panels it would read into belong to another repository by now. */
  #cleared = 0;

  get total(): number {
    return this.staged.length + this.unstaged.length;
  }

  async load(repo: RepoId): Promise<void> {
    const generation = ++this.#generation;
    this.error = null;
    this.loading = true;

    try {
      const files = await worktreeFiles(repo, backendView(filesView.current));
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

  async stage(repo: RepoId, paths: string[]): Promise<void> {
    await this.mutate(repo, () => stagePaths(repo, paths));
  }

  async unstage(repo: RepoId, paths: string[]): Promise<void> {
    await this.mutate(repo, () => unstagePaths(repo, paths));
  }

  async discard(repo: RepoId, paths: string[]): Promise<void> {
    await this.mutate(repo, () => discardPaths(repo, paths));
  }

  /** A mutation is only believed once the working tree has been read back. */
  async mutate(repo: RepoId, run: () => Promise<unknown>): Promise<void> {
    this.error = null;
    const cleared = this.#cleared;
    try {
      await run();
    } catch (err) {
      if (cleared !== this.#cleared) return;
      this.error =
        err instanceof CogitError ? err : new CogitError({ kind: "internal", data: String(err) });
      return;
    }
    if (cleared === this.#cleared) await this.load(repo);
  }

  async commit(
    repo: RepoId,
    message: string,
    amend: boolean,
    noVerify: boolean,
    only: string[] = [],
  ): Promise<void> {
    await this.mutate(repo, () => createCommit(repo, { message, amend, noVerify, only }));
  }

  clear(): void {
    this.#generation += 1;
    this.#cleared += 1;
    this.staged = [];
    this.unstaged = [];
    this.loading = false;
    this.error = null;
  }
}

export const worktree = new WorktreeStore();
