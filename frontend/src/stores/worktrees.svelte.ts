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

  async refresh(repo: RepoId): Promise<void> {
    this.repo = repo;
    try {
      this.entries = await listWorktrees(repo);
    } catch {
      this.entries = [];
    }
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

  async repair(path: string): Promise<void> {
    await this.#act((repo) => repairWorktree(repo, path));
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
    this.entries = [];
    this.repo = null;
    this.selected = null;
  }

  async #act(run: (repo: RepoId) => Promise<unknown>): Promise<void> {
    const repo = this.repo;
    if (repo === null) return;
    try {
      await run(repo);
    } finally {
      await this.refresh(repo);
    }
  }
}

export const worktrees = new WorktreesStore();
