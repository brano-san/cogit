import {
  type CogitError,
  diffFile,
  imageSides,
  type DiffSpec,
  type FileDiff,
  type Hunk,
  type Whitespace,
  type RepoId,
  toCogitError,
} from "$lib/ipc";
import { readKey, writeKey } from "$lib/settings-file";
import { discardSelection, stageSelection, type PatchRequest } from "$lib/ipc";
import { expandedContext } from "$lib/diff-rows";
import { splitSelection } from "$lib/selection";
import { settings } from "./settings.svelte";

const DEFAULT_CONTEXT = 3;

/** The branch keeps its own key in the shared store file: `Settings` belongs to `master`. */
const PREFS_KEY = "diffView";

export type DiffLayout = "unified" | "split";

const LAYOUTS: readonly DiffLayout[] = ["unified", "split"];

class DiffStore {
  path = $state<string | null>(null);
  diff = $state.raw<FileDiff | null>(null);
  loading = $state(false);
  error = $state<CogitError | null>(null);
  spec = $state.raw<DiffSpec | null>(null);
  /** Remembered per repository: the mode outlives switching between files. */
  whitespace = $state<Whitespace>("none");
  context = $state<number | null>(null);
  images = $state.raw<[string | null, string | null]>([null, null]);
  /** Remembered between runs, unlike `whitespace`, which is a per-session mode. */
  layout = $state<DiffLayout>("split");
  /** Off shows a moved block as an ordinary deletion plus addition (T7.9). */
  showMoves = $state(true);

  #prefsRead = false;
  /** Which repository the shown diff came from, so a toggle can recompute it. */
  #repo: RepoId | null = null;
  /** The file `diff` belongs to. `path` and `spec` move to the next file at once; this
      stays with the lines on screen until that file's diff arrives. */
  #shown = $state.raw<{ repo: RepoId; path: string; spec: DiffSpec } | null>(null);

  /** The file the lines on screen belong to; `path` is the one asked for last. */
  get shownPath(): string | null {
    return this.#shown?.path ?? null;
  }

  get hunks(): Hunk[] {
    return this.diff?.kind === "text" ? this.diff.hunks : [];
  }

  /** Which repository the shown diff came from; views in `components/diff/**` have no
      other way to learn it, because the panel above them belongs to `master`. */
  get repo(): RepoId | null {
    return this.#repo;
  }

  /** Lines kept around a change when the view folds a diff that carries the whole file. */
  get foldContext(): number {
    return settings.diffOptions.contextLines;
  }

  /** Which of the line actions make sense on the side of the index on screen: the
      working tree against the index stages and discards, the index against HEAD only
      unstages. The patch is cut from this diff, so on the other side it means other lines. */
  get lineActions(): { stage: boolean; unstage: boolean; discard: boolean } {
    const kind = this.spec?.kind;
    return {
      stage: kind === "workTreeVsIndex",
      unstage: kind === "indexVsHead",
      discard: kind === "workTreeVsIndex",
    };
  }

  get stageable(): boolean {
    return this.spec?.kind === "workTreeVsIndex" || this.spec?.kind === "indexVsHead";
  }

  /** This file under this spec is on screen already, so clicking it again changes nothing. */
  shows(spec: DiffSpec, path: string): boolean {
    return this.path === path && this.error === null && sameSpec(this.spec, spec);
  }

  /** Clicking down the file list outruns the backend; stale diffs lose. */
  #generation = 0;

  async load(repo: RepoId, spec: DiffSpec, path: string): Promise<void> {
    const generation = ++this.#generation;
    if (path !== this.path) this.context = null;
    this.#repo = repo;
    this.path = path;
    this.spec = spec;
    this.error = null;
    this.loading = true;

    try {
      const result = await diffFile(repo, spec, path, {
        ...settings.diffOptions,
        ...(this.context === null ? {} : { contextLines: this.context }),
        ignoreWhitespace: this.whitespace,
        detectMoves: settings.diffOptions.detectMoves && this.showMoves,
      });
      if (generation !== this.#generation) return;
      const images = result.kind === "image" ? await imageSides(repo, spec, path) : null;
      if (generation !== this.#generation) return;
      this.diff = result;
      this.images = images ?? [null, null];
      this.#shown = { repo, path, spec };
    } catch (err) {
      if (generation !== this.#generation) return;
      this.diff = null;
      this.#shown = null;
      this.error = toCogitError(err);
    } finally {
      if (generation === this.#generation) this.loading = false;
    }
  }

  async expand(repo: RepoId, whole: boolean): Promise<void> {
    if (!this.spec || !this.path) return;
    this.context = expandedContext(this.context ?? DEFAULT_CONTEXT, whole);
    await this.load(repo, this.spec, this.path);
  }

  /** Recomputes this file and nothing else: the panel keeps its scroll and selection. */
  async reload(): Promise<void> {
    if (this.#repo === null || !this.spec || !this.path) return;
    await this.load(this.#repo, this.spec, this.path);
  }

  /** Throws the selected lines away in the working tree. The caller confirms first.
      `from` is the diff the lines were chosen in; once another has replaced it, their
      numbers mean other lines, and nothing is thrown away. */
  async discardLines(selected: ReadonlySet<string>, from?: FileDiff): Promise<void> {
    if (from !== undefined && from !== this.diff) return;
    // Discard reverses the patch in the working tree: only lines of that diff can go.
    if (!this.lineActions.discard) return;
    const lines = this.#lines(selected);
    if (!lines) return;
    await discardSelection(lines.repo, lines.request);
    await this.reload();
  }

  /** Stages (or, `reverse`, unstages) the selected lines of the diff on screen. The caller
      reloads what the change touched, as for any other mutation. */
  async stageLines(selected: ReadonlySet<string>, reverse: boolean): Promise<void> {
    if (!(reverse ? this.lineActions.unstage : this.lineActions.stage)) return;
    const lines = this.#lines(selected);
    if (!lines) return;
    await stageSelection(lines.repo, lines.request, reverse);
  }

  /** The request for lines chosen in the diff on screen: its file and its hunks, never the
      file asked for next. */
  #lines(selected: ReadonlySet<string>): { repo: RepoId; request: PatchRequest } | null {
    const shown = this.#shown;
    if (shown === null || this.diff?.kind !== "text") return null;
    const { deletes, inserts } = splitSelection(selected);
    if (deletes.length === 0 && inserts.length === 0) return null;
    return {
      repo: shown.repo,
      request: {
        path: shown.path,
        hunks: this.diff.hunks,
        selectedDeletes: deletes,
        selectedInserts: inserts,
      },
    };
  }

  /**
   * A mutation touched these paths.
   *
   * The shown file is re-diffed rather than blanked: staging one hunk must not cost the
   * user their place in the file (T6.7). It still clears when there is nothing left to
   * show — the change was staged whole, or the file is gone.
   *
   * Returns the promise so tests can wait for it; callers fire and forget.
   */
  async dropIfAffected(paths: readonly string[]): Promise<void> {
    if (this.path === null || !paths.includes(this.path)) return;
    await this.reload();
    if (this.error !== null || this.diff === null || this.diff.kind === "unchanged") {
      this.clear();
    }
  }

  /** Reads the remembered view once per session; every view may ask. */
  async loadPreferences(): Promise<void> {
    if (this.#prefsRead) return;
    this.#prefsRead = true;
    try {
      const saved = await readKey<unknown>(PREFS_KEY);
      if (typeof saved !== "object" || saved === null) return;
      const { layout, showMoves } = saved as Partial<Record<string, unknown>>;
      if (LAYOUTS.includes(layout as DiffLayout)) this.layout = layout as DiffLayout;
      if (typeof showMoves === "boolean") this.showMoves = showMoves;
    } catch {
      // Unreadable store: a fresh install or a locked profile. The defaults still work.
    }
  }

  async setLayout(next: DiffLayout): Promise<void> {
    this.layout = next;
    await this.#writePreferences();
  }

  /** Re-runs the diff: whether a block reads as moved is decided in Rust, not here.
      Takes no repository — the view that offers the toggle does not know it, and the
      store already learned it from the last `load`. */
  async setShowMoves(next: boolean): Promise<void> {
    this.showMoves = next;
    await this.#writePreferences();
    if (this.#repo !== null && this.spec && this.path) {
      await this.load(this.#repo, this.spec, this.path);
    }
  }

  async #writePreferences(): Promise<void> {
    try {
      await writeKey(PREFS_KEY, { layout: this.layout, showMoves: this.showMoves });
    } catch {
      // The choice still holds for this session even when it cannot be written down.
    }
  }

  /** Tests only: the store is a singleton, so preferences outlive `clear()`. */
  resetPreferences(): void {
    this.#prefsRead = false;
    this.layout = "split";
    this.showMoves = true;
  }

  /** Re-runs the last diff with the new mode so the panel does not go stale. */
  async setWhitespace(repo: RepoId, mode: Whitespace): Promise<void> {
    this.whitespace = mode;
    if (this.spec && this.path) await this.load(repo, this.spec, this.path);
  }

  clear(): void {
    this.#generation += 1;
    this.#repo = null;
    this.#shown = null;
    this.path = null;
    this.spec = null;
    this.context = null;
    this.diff = null;
    this.images = [null, null];
    this.loading = false;
    this.error = null;
  }
}

function sameSpec(a: DiffSpec | null, b: DiffSpec): boolean {
  if (a === null || a.kind !== b.kind) return false;
  if (a.kind === "commitVsParent" && b.kind === "commitVsParent") return a.oid === b.oid;
  if (a.kind === "commitVsCommit" && b.kind === "commitVsCommit") return a.a === b.a && a.b === b.b;
  return true;
}

export const diff = new DiffStore();
