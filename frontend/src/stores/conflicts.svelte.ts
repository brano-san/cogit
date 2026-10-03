import {
  conflictedPaths,
  conflictText,
  mergePreview,
  resolveConflict,
  resolveConflictText,
  rerereForget,
  rerereStatus,
  type ConflictSide,
  type ConflictStages,
  type EntryKind,
  type RerereStatus,
  type Region,
  type RepoId,
} from "$lib/ipc";
import { confirmation } from "./confirm.svelte";
import { notices } from "$stores/notices.svelte";

class ConflictStore {
  paths = $state.raw<string[]>([]);
  path = $state<string | null>(null);
  base = $state<string | null>(null);
  ours = $state<string | null>(null);
  theirs = $state<string | null>(null);
  /** A side is binary or not UTF-8: only taking one side whole can resolve it. */
  binary = $state(false);
  /** A link or a submodule is `binary` too, but it is named as what it is. */
  kind = $state<EntryKind>("regular");
  /** The index entries the sides on screen were read from; a write names them back. */
  stages = $state.raw<ConflictStages | null>(null);
  /** Over the size limit: no side was sent and none is merged. */
  tooLarge = $state(false);
  missingOurs = $state(false);
  missingTheirs = $state(false);
  /** The three sides already merged; empty until a conflicted file is opened. */
  regions = $state.raw<Region[]>([]);
  /** Sides picked or text edited in the view on screen, not written yet. The views say so
      through `markUnsaved`; only they hold the picks. */
  unsaved = $state(false);
  /** What rerere did with the conflicts listed; `null` while there are none (D6). */
  rerere = $state.raw<RerereStatus | null>(null);
  /** The repository `paths` was read for; `null` until it is: an empty list after `clear()` is
      "not read yet", not "no conflicts". */
  listedFor = $state.raw<RepoId | null>(null);

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
    this.listedFor = repo;
    const rerere = paths.length > 0 ? await rerereStatus(repo).catch(() => null) : null;
    if (listing !== this.#listing) return;
    this.rerere = rerere;
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

    let sides;
    try {
      sides = await conflictText(repo, path);
    } catch (err) {
      if (generation === this.#generation) notices.report(err, "Could not open the conflict");
      return;
    }
    if (generation !== this.#generation) return;

    this.path = path;
    this.base = sides.base;
    this.ours = sides.ours;
    this.theirs = sides.theirs;
    this.binary = sides.binary;
    this.kind = sides.kind;
    this.stages = sides.stages;
    this.tooLarge = sides.tooLarge;
    this.missingOurs = sides.missingOurs;
    this.missingTheirs = sides.missingTheirs;
    this.regions = [];
    if (sides.binary || sides.tooLarge) return;

    // A failed merge leaves the three raw sides, which are still worth showing.
    const regions = await mergePreview(repo, path).catch(() => []);
    if (generation !== this.#generation) return;
    this.regions = regions;
  }

  async take(repo: RepoId, side: ConflictSide): Promise<void> {
    await this.#resolve(repo, (path, stages) => resolveConflict(repo, path, side, stages));
  }

  async write(repo: RepoId, text: string): Promise<void> {
    await this.#resolve(repo, (path, stages) => resolveConflictText(repo, path, text, stages));
  }

  /** A refusal keeps the file open with its picks and reaches the notification window:
      the callers only reload the list after it. A conflict redone since the view was
      built closes it, so the next click reads the file again. */
  async #resolve(
    repo: RepoId,
    step: (path: string, stages: ConflictStages | null) => Promise<unknown>,
  ): Promise<void> {
    const path = this.path;
    if (!path) return;
    const stages = this.stages;
    const cleared = this.#cleared;
    try {
      await step(path, stages);
    } catch (err) {
      notices.report(err, "Could not resolve the conflict");
      const said = err instanceof Error ? err.message : JSON.stringify(err);
      if (said.includes("changed since") && this.path === path) this.close();
      return;
    }
    if (cleared !== this.#cleared) return;
    // The user may have opened the next file while Git was answering; that one stays.
    if (this.path === path) this.close();
    await this.refresh(repo);
  }

  /** The merge window saved `path`; the main window may be showing another file by now,
      with sides picked in it. */
  resolvedElsewhere(path: string): void {
    if (this.path === path) this.close();
  }

  /** rerere resolved `path` from a recorded resolution. */
  autoResolved(path: string): boolean {
    return this.rerere?.resolved.includes(path) ?? false;
  }

  /** `git rerere forget`: the recorded resolution goes and the conflict markers come back. */
  async forget(path: string): Promise<void> {
    const repo = this.listedFor;
    if (!repo) return;
    try {
      await rerereForget(repo, [path]);
    } catch (err) {
      notices.report(err, "Could not forget the resolution");
      return;
    }
    if (this.path === path) this.close();
    await this.refresh(repo);
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
    this.kind = "regular";
    this.stages = null;
    this.tooLarge = false;
    this.missingOurs = false;
    this.missingTheirs = false;
    this.regions = [];
  }

  clear(): void {
    this.#listing += 1;
    this.#cleared += 1;
    this.paths = [];
    this.rerere = null;
    this.listedFor = null;
    this.close();
  }
}

export const conflicts = new ConflictStore();
