import { backendView } from "$lib/file-view";
import { stagesEverything } from "$lib/stage-all";
import { filesView } from "$stores/files-view.svelte";
import {
  type CogitError,
  createCommit,
  discardPaths,
  stageAll,
  stagePaths,
  unstagePaths,
  worktreeFiles,
  type FileEntry,
  type WorkingState,
  type RepoId,
  toCogitError,
} from "$lib/ipc";

class WorktreeStore {
  staged = $state.raw<FileEntry[]>([]);
  unstaged = $state.raw<FileEntry[]>([]);
  loading = $state(false);
  /** An answer came since `clear()`: until then an empty list says nothing about the tree. */
  loaded = $state(false);
  error = $state<CogitError | null>(null);

  #generation = 0;
  /** Bumped by `clear()`: a write that finishes after it reads nothing back, since the
      panels it would read into belong to another repository by now. */
  #cleared = 0;

  get total(): number {
    return this.staged.length + this.unstaged.length;
  }

  /** The counters and conflicted paths of the same read; `null` when it failed or an
      answer for another read got there first. */
  async load(repo: RepoId): Promise<WorkingState | null> {
    const generation = ++this.#generation;
    this.error = null;
    this.loading = true;

    try {
      const files = await worktreeFiles(repo, backendView(filesView.current));
      if (generation !== this.#generation) return null;
      this.staged = files.staged;
      this.unstaged = files.unstaged;
      this.loaded = true;
      return files.state;
    } catch (err) {
      if (generation !== this.#generation) return null;
      this.staged = [];
      this.unstaged = [];
      this.error = toCogitError(err);
      return null;
    } finally {
      if (generation === this.#generation) this.loading = false;
    }
  }

  async stage(repo: RepoId, paths: string[]): Promise<WorkingState | null> {
    const everything = stagesEverything(paths, this.unstaged);
    return this.mutate(repo, () => (everything ? stageAll(repo, paths.length) : stagePaths(repo, paths)));
  }

  async unstage(repo: RepoId, paths: string[]): Promise<WorkingState | null> {
    return this.mutate(repo, () => unstagePaths(repo, paths));
  }

  async discard(repo: RepoId, paths: string[]): Promise<WorkingState | null> {
    return this.mutate(repo, () => discardPaths(repo, paths));
  }

  /** A mutation is only believed once the working tree has been read back. A refused write
      is thrown to the caller, which reports it as the write it was; `error` is only ever a
      failed read, so a read failing after a good write does not undo the write's success. */
  async mutate(repo: RepoId, run: () => Promise<unknown>): Promise<WorkingState | null> {
    const cleared = this.#cleared;
    try {
      await run();
    } catch (err) {
      // Thrown even after the panels moved on: the caller must not take a refusal for a
      // commit. Only the read-back below is skipped for another repository.
      throw toCogitError(err);
    }
    return cleared === this.#cleared ? this.load(repo) : null;
  }

  async commit(
    repo: RepoId,
    message: string,
    amend: boolean,
    noVerify: boolean,
    only: string[] = [],
    signoff = false,
  ): Promise<void> {
    await this.mutate(repo, () => createCommit(repo, { message, amend, noVerify, only, signoff }));
  }

  clear(): void {
    this.#generation += 1;
    this.#cleared += 1;
    this.staged = [];
    this.unstaged = [];
    this.loading = false;
    this.loaded = false;
    this.error = null;
  }
}

export const worktree = new WorktreeStore();
