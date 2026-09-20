import {
  addWorktree,
  listWorktrees,
  pruneWorktrees,
  removeWorktree,
  worktreeHolding,
  type RepoId,
  type WorktreeEntry,
} from "$lib/ipc";

class WorktreesStore {
  entries = $state.raw<WorktreeEntry[]>([]);

  async refresh(repo: RepoId): Promise<void> {
    try {
      this.entries = await listWorktrees(repo);
    } catch {
      // A repository that cannot list its worktrees simply has none to show.
      this.entries = [];
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

  async add(repo: RepoId, path: string, branch: string, create: boolean): Promise<void> {
    await addWorktree(repo, path, branch, create);
    await this.refresh(repo);
  }

  async remove(repo: RepoId, path: string, force: boolean): Promise<void> {
    await removeWorktree(repo, path, force);
    await this.refresh(repo);
  }

  async prune(repo: RepoId): Promise<void> {
    await pruneWorktrees(repo);
    await this.refresh(repo);
  }

  clear(): void {
    this.entries = [];
  }
}

export const worktrees = new WorktreesStore();
