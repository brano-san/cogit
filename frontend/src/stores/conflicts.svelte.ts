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
import { confirmation } from "./confirm.svelte";

class ConflictStore {
  paths = $state.raw<string[]>([]);
  path = $state<string | null>(null);
  base = $state<string | null>(null);
  ours = $state<string | null>(null);
  theirs = $state<string | null>(null);
  /** A side is binary or not UTF-8: only taking one side whole can resolve it. */
  binary = $state(false);
  /** The three sides already merged; empty until a conflicted file is opened. */
  regions = $state.raw<Region[]>([]);
  /** Sides picked or text edited in the view on screen, not written yet. The views say so
      through `markUnsaved`; only they hold the picks. */
  unsaved = $state(false);

  /** Opening a file takes two round trips. Without this, a slow answer for the file the
      user has moved on from lands on the file they are looking at now — and Save would
      then write one file's resolution into another. */
  #generation = 0;
  /** The same for the list, which `clear()` also drops when the panels change repository. */
  #listing = 0;
  /** Bumped by `clear()`: a write that finishes after it reads nothing back, since the
      panels it would read into belong to another repository by now. */
  #cleared = 0;
  /** The file the newest `open` is for, on screen already or still being read. */
  #wanted: string | null = null;

  /** `known`: the list a status read of the same moment already brought (R-316). */
  async refresh(repo: RepoId, known?: readonly string[]): Promise<void> {
    const listing = ++this.#listing;
    const paths = known ? [...known] : await conflictedPaths(repo);
    if (listing !== this.#listing) return;
    this.paths = paths;
    if (this.path && !this.paths.includes(this.path)) this.close();
  }

  /** The file on screen is not opened again: that would rebuild the view and drop the
      sides picked in it. Another file opening meanwhile loses to it. */
  async open(repo: RepoId, path: string): Promise<void> {
    if (this.path === path) {
      if (this.#wanted !== path) {
        this.#generation += 1;
        this.#wanted = path;
      }
      return;
    }
    const generation = ++this.#generation;
    this.#wanted = path;

    const sides = await conflictText(repo, path);
    if (generation !== this.#generation) return;

    this.path = path;
    this.base = sides.base;
    this.ours = sides.ours;
    this.theirs = sides.theirs;
    this.binary = sides.binary;
    this.regions = [];
    if (sides.binary) return;

    // A failed merge leaves the three raw sides, which are still worth showing.
    const regions = await mergePreview(repo, path).catch(() => []);
    if (generation !== this.#generation) return;
    this.regions = regions;
  }

  async take(repo: RepoId, side: ConflictSide): Promise<void> {
    const path = this.path;
    if (!path) return;
    const cleared = this.#cleared;
    await resolveConflict(repo, path, side);
    if (cleared !== this.#cleared) return;
    // The user may have opened the next file while Git was answering; that one stays.
    if (this.path === path) this.close();
    await this.refresh(repo);
  }

  async write(repo: RepoId, text: string): Promise<void> {
    const path = this.path;
    if (!path) return;
    const cleared = this.#cleared;
    await resolveConflictText(repo, path, text);
    if (cleared !== this.#cleared) return;
    if (this.path === path) this.close();
    await this.refresh(repo);
  }

  /** The merge window saved `path`; the main window may be showing another file by now,
      with sides picked in it. */
  resolvedElsewhere(path: string): void {
    if (this.path === path) this.close();
  }

  markUnsaved(unsaved: boolean): void {
    this.unsaved = unsaved && this.path !== null;
  }

  /** Before another file takes the Diff panel: false while the user keeps the merge. */
  async leave(): Promise<boolean> {
    const path = this.path;
    if (path === null) return true;
    if (this.unsaved) {
      const go = await confirmation.ask({
        title: "Discard the Resolution",
        message: `The sides picked and the edits to ${path} are not saved. Leave the merge and lose them?`,
        confirm: "Discard",
        warning: true,
      });
      if (!go) return false;
    }
    if (this.path === path) this.close();
    return true;
  }

  /** Another commit picked in the graph: a merge with nothing picked in it gives way, one
      with picks stays until a file is opened in its place, which asks. */
  closeUnlessUnsaved(): void {
    if (!this.unsaved) this.close();
  }

  close(): void {
    this.#generation += 1;
    this.#wanted = null;
    this.unsaved = false;
    this.path = null;
    this.base = null;
    this.ours = null;
    this.theirs = null;
    this.binary = false;
    this.regions = [];
  }

  clear(): void {
    this.#listing += 1;
    this.#cleared += 1;
    this.paths = [];
    this.close();
  }
}

export const conflicts = new ConflictStore();
