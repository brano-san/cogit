import {
  conflictedPaths,
  conflictText,
  resolveConflict,
  resolveConflictText,
  type ConflictSide,
  type RepoId,
} from "$lib/ipc";

class ConflictStore {
  paths = $state.raw<string[]>([]);
  path = $state<string | null>(null);
  base = $state<string | null>(null);
  ours = $state<string | null>(null);
  theirs = $state<string | null>(null);

  async refresh(repo: RepoId): Promise<void> {
    this.paths = await conflictedPaths(repo);
    if (this.path && !this.paths.includes(this.path)) this.close();
  }

  async open(repo: RepoId, path: string): Promise<void> {
    const sides = await conflictText(repo, path);
    this.path = path;
    this.base = sides.base;
    this.ours = sides.ours;
    this.theirs = sides.theirs;
  }

  async take(repo: RepoId, side: ConflictSide): Promise<void> {
    if (!this.path) return;
    await resolveConflict(repo, this.path, side);
    this.close();
    await this.refresh(repo);
  }

  async write(repo: RepoId, text: string): Promise<void> {
    if (!this.path) return;
    await resolveConflictText(repo, this.path, text);
    this.close();
    await this.refresh(repo);
  }

  close(): void {
    this.path = null;
    this.base = null;
    this.ours = null;
    this.theirs = null;
  }

  clear(): void {
    this.paths = [];
    this.close();
  }
}

export const conflicts = new ConflictStore();
