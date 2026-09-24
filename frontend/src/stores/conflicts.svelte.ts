import {
  conflictedPaths,
  conflictText,
  mergePreview,
  resolveConflict,
  resolveConflictText,
  type ConflictSide,
  type Region,
  type RepoId,
} from "$lib/ipc";

class ConflictStore {
  paths = $state.raw<string[]>([]);
  path = $state<string | null>(null);
  base = $state<string | null>(null);
  ours = $state<string | null>(null);
  theirs = $state<string | null>(null);
  /** The three sides already merged; empty until a conflicted file is opened. */
  regions = $state.raw<Region[]>([]);

  /** Opening a file takes two round trips. Without this, a slow answer for the file the
      user has moved on from lands on the file they are looking at now — and Save would
      then write one file's resolution into another. */
  #generation = 0;
  /** The same for the list, which `clear()` also drops when the panels change repository. */
  #listing = 0;

  async refresh(repo: RepoId): Promise<void> {
    const listing = ++this.#listing;
    const paths = await conflictedPaths(repo);
    if (listing !== this.#listing) return;
    this.paths = paths;
    if (this.path && !this.paths.includes(this.path)) this.close();
  }

  async open(repo: RepoId, path: string): Promise<void> {
    const generation = ++this.#generation;

    const sides = await conflictText(repo, path);
    if (generation !== this.#generation) return;

    this.path = path;
    this.base = sides.base;
    this.ours = sides.ours;
    this.theirs = sides.theirs;
    this.regions = [];

    // A failed merge leaves the three raw sides, which are still worth showing.
    const regions = await mergePreview(repo, path).catch(() => []);
    if (generation !== this.#generation) return;
    this.regions = regions;
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
    this.#generation += 1;
    this.path = null;
    this.base = null;
    this.ours = null;
    this.theirs = null;
    this.regions = [];
  }

  clear(): void {
    this.#listing += 1;
    this.paths = [];
    this.close();
  }
}

export const conflicts = new ConflictStore();
