import {
  addWorktree,
  listWorktrees,
  lockWorktree,
  pruneWorktree,
  pruneWorktrees,
  removeWorktree,
  repairWorktree,
  unlockWorktree,
  worktreeChanges,
  worktreeHolding,
  type FileEntry,
  type RepoId,
  type WorktreeEntry,
} from "$lib/ipc";
import { repoNameOf } from "$lib/notices";
import { notices } from "$stores/notices.svelte";

class WorktreesStore {
  entries = $state.raw<WorktreeEntry[]>([]);
  /** The repository the list was read from; every command goes to it. */
  repo = $state<RepoId | null>(null);
  /** The row clicked last, which Repository ▸ Remove Worktree… acts on. */
  selected = $state<string | null>(null);
  /** The listed repository a worktree was opened from, marked in Repositories (R-184). */
  ownerRoot = $state<string | null>(null);

  get current(): WorktreeEntry | undefined {
    return this.entries.find((entry) => entry.path === this.selected);
  }

  /** Only the newest read writes; `clear()` drops the ones in flight, which belong to the
      repository the panels are leaving. */
  #generation = 0;
  /** Bumped by `clear()`: a write that finishes after it reads nothing back, since the
      panels it would read into belong to another repository by now. */
  #cleared = 0;

  async refresh(repo: RepoId): Promise<void> {
    const generation = ++this.#generation;
    this.repo = repo;
    const entries = await listWorktrees(repo).catch(() => []);
    if (generation !== this.#generation) return;
    this.entries = entries;
    if (this.selected && !this.entries.some((entry) => entry.path === this.selected)) {
      this.selected = null;
    }
  }

  /** Which worktree already holds a branch, so a checkout can offer to go there (T3.8). */
  async holding(repo: RepoId, branch: string): Promise<WorktreeEntry | null> {
    try {
      return await worktreeHolding(repo, branch);
    } catch {
      return null;
    }
  }

  async add(path: string, branch: string, create: boolean, base: string | null): Promise<void> {
    await this.#act((repo) => addWorktree(repo, path, branch, create, base));
  }

  async remove(path: string, force: boolean): Promise<void> {
    await this.#act((repo) => removeWorktree(repo, path, force));
  }

  async prune(): Promise<void> {
    await this.#act((repo) => pruneWorktrees(repo));
  }

  async pruneOne(path: string): Promise<void> {
    await this.#act((repo) => pruneWorktree(repo, path));
  }

  /** Queued like every write, so the footer says so; how it ended is a notification. */
  async repair(path: string): Promise<void> {
    await this.#act((repo) => repairWorktree(repo, path));
    notices.inform("Worktree repaired", `Git knows ${repoNameOf(path)} is at ${path} again.`);
  }

  async lock(path: string, reason: string | null): Promise<void> {
    await this.#act((repo) => lockWorktree(repo, path, reason));
  }

  async unlock(path: string): Promise<void> {
    await this.#act((repo) => unlockWorktree(repo, path));
  }

  async changes(path: string): Promise<FileEntry[]> {
    return this.repo === null ? [] : await worktreeChanges(this.repo, path);
  }

  clear(): void {
    this.#generation += 1;
    this.entries = [];
    this.repo = null;
    this.selected = null;
    this.#cleared += 1;
  }

  async #act(run: (repo: RepoId) => Promise<unknown>): Promise<void> {
    const repo = this.repo;
    if (repo === null) return;
    const cleared = this.#cleared;
    try {
      await run(repo);
    } finally {
      if (cleared === this.#cleared) await this.refresh(repo);
    }
  }
}

export const worktrees = new WorktreesStore();
