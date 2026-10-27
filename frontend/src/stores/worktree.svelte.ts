import { untrack } from "svelte";
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
  /** A filter searches a clean working tree: the unchanged files are read as well, into
      `found` — listed, never counted or acted on as changes. */
  everyFile = $state(false);
  found = $state.raw<FileEntry[]>([]);

  #generation = 0;
  /** Reads started; what a stopped search found stays good until the next one. */
  #reads = 0;
  /** A stopped search's files: typing again over the same tree lists them at once, without
      reading every file and sorting them again (R-823). */
  #kept: { found: FileEntry[]; reads: number } | null = null;
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
    this.#reads += 1;
    this.error = null;
    this.loading = true;

    try {
      // Untracked: an effect that loads would otherwise run again on every Files switch.
      const [view, apart] = untrack(() => [backendView(filesView.current), this.everyFile && !filesView.current.unchanged]);
      const files = await worktreeFiles(repo, apart ? { ...view, unchanged: true } : view);
      if (generation !== this.#generation) return null;
      this.staged = files.staged;
      this.unstaged = apart ? files.unstaged.filter((file) => file.status !== "unchanged") : files.unstaged;
      // The search may have stopped while the read was on its way.
      this.found = apart && this.everyFile ? files.unstaged.filter((file) => file.status === "unchanged") : [];
      this.loaded = true;
      return files.state;
    } catch (err) {
      if (generation !== this.#generation) return null;
      this.staged = [];
      this.unstaged = [];
      this.found = [];
      this.error = toCogitError(err);
      return null;
    } finally {
      if (generation === this.#generation) this.loading = false;
    }
  }

  /** Starts or stops the search over every file; stopping only drops what it found. */
  async searchEveryFile(on: boolean, repo: RepoId | null): Promise<void> {
    if (this.everyFile === on) return;
    this.everyFile = on;
    if (!on) {
      if (this.found.length > 0) this.#kept = { found: this.found, reads: this.#reads };
      this.found = [];
    } else if (this.#kept !== null && this.#kept.reads === this.#reads) {
      this.found = this.#kept.found;
    } else if (repo !== null) {
      await this.load(repo);
    }
  }

  /** `exact` names every path even when they are all of Unstaged: a list read a while ago
      may be missing files that `add --all` would take. */
  async stage(repo: RepoId, paths: string[], exact = false): Promise<WorkingState | null> {
    const everything = !exact && stagesEverything(paths, this.unstaged);
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
    const readBack = () => (cleared === this.#cleared ? this.load(repo) : Promise.resolve(null));
    try {
      await run();
    } catch (err) {
      // Read back all the same: git can change the index and then exit non-zero (`add`
      // with an ignored path). Thrown even after the panels moved on: the caller must not
      // take a refusal for a commit.
      await readBack();
      throw toCogitError(err);
    }
    return readBack();
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
    this.found = [];
    this.#kept = null;
    this.everyFile = false;
    this.loading = false;
    this.loaded = false;
    this.error = null;
  }
}

export const worktree = new WorktreeStore();
