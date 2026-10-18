import {
  type CogitError,
  diffFile,
  imageSides,
  type DiffOptions,
  type DiffSpec,
  type FileDiff,
  type Hunk,
  type Whitespace,
  type RepoId,
  toCogitError,
} from "$lib/ipc";
import { readKey, writeKey } from "$lib/settings-file";
import { discardSelection, stageSelection, type PatchRequest } from "$lib/ipc";
import { WHOLE_FILE, expandedContext } from "$lib/diff-rows";
import { splitSelection } from "$lib/selection";
import { runMutation, type MutationContext } from "$lib/mutation";
import { isAbsentFromBothSides } from "$lib/diff-toolbar";
import { settings } from "./settings.svelte";
import { diffEdit, type Opened } from "./diff-edit.svelte";
import { diffEdited } from "$lib/ipc/editable";

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
  /** The file is gone from both sides (deleted from disk, never in the index): a state of
      its own, not a failure. Nothing re-reads it until the user opens it again. */
  gone = $state(false);
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
  /** How App ends a change to the working tree; see `useMutation`. */
  #mutation: MutationContext | null = null;
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
      unstages. The patch is cut from this diff, so on the other side it means other lines.
      The diff on screen decides, not the one asked for next: until it arrives, the lines
      and hunks are the old one's. */
  get lineActions(): { stage: boolean; unstage: boolean; discard: boolean } {
    const kind = this.#shown?.spec.kind;
    return {
      stage: kind === "workTreeVsIndex",
      unstage: kind === "indexVsHead",
      discard: kind === "workTreeVsIndex",
    };
  }

  /** The spec of the lines on screen; `spec` moves to the next file at once. */
  get shownSpec(): DiffSpec | null {
    return this.#shown?.spec ?? null;
  }

  get stageable(): boolean {
    const kind = this.#shown?.spec.kind;
    return kind === "workTreeVsIndex" || kind === "indexVsHead";
  }

  /** This file under this spec is on screen already, so clicking it again changes nothing. */
  shows(spec: DiffSpec, path: string): boolean {
    return this.path === path && this.error === null && !this.gone && sameSpec(this.spec, spec);
  }

  /** The file asked for last, if it was asked for under `spec`: which list row is open. */
  pathFor(spec: DiffSpec | null): string | null {
    return spec !== null && sameSpec(this.spec, spec) ? this.path : null;
  }

  /** Clicking down the file list outruns the backend; stale diffs lose. */
  #generation = 0;

  /** Another file while the editor has unsaved edits: Save, Discard or stay (F-722). */
  async load(repo: RepoId, spec: DiffSpec, path: string): Promise<void> {
    if (!(await diffEdit.leaveFor(path, spec))) return;
    const generation = ++this.#generation;
    if (path !== this.path) this.context = null;
    this.#repo = repo;
    this.path = path;
    this.spec = spec;
    this.error = null;
    this.gone = false;
    this.loading = true;

    try {
      // The whole file shown: asked for at once, not after the folded diff arrives.
      const context = this.context ?? (settings.current.diffWholeFile ? WHOLE_FILE : null);
      const options: DiffOptions = {
        ...settings.diffOptions,
        ...(context === null ? {} : { contextLines: context }),
        ignoreWhitespace: this.whitespace,
        detectMoves: settings.diffOptions.detectMoves && this.showMoves,
      };
      const open = diffEdit.openFor(repo, spec, path);
      const revision = diffEdit.revision;
      const result = open ? await this.#edited(repo, spec, open, options) : await diffFile(repo, spec, path, options);
      if (generation !== this.#generation) return;
      // Typed since this text was sent: the diff of the newer one is on its way.
      if (open && revision !== diffEdit.revision) return;
      const images = result.kind === "image" ? await imageSides(repo, spec, path) : null;
      if (generation !== this.#generation) return;
      this.diff = result;
      this.images = images ?? [null, null];
      // The same file again keeps its `#shown`: a view reading it would start over (scroll,
      // selection, open folds) on every re-diff of it: a stage, a discard, a change on disk.
      const shown = this.#shown;
      if (shown?.repo !== repo || shown.path !== path || !sameSpec(shown.spec, spec)) this.#shown = { repo, path, spec };
    } catch (err) {
      if (generation !== this.#generation) return;
      this.diff = null;
      this.#shown = null;
      const failure = toCogitError(err);
      if (isAbsentFromBothSides(failure) && WORKING_SPECS.includes(spec.kind)) this.gone = true;
      else this.error = failure;
    } finally {
      if (generation === this.#generation) this.loading = false;
    }
  }

  /** The file as the editor holds it (F-722). Typed back to the base, the edits read as no
      change at all: the lines stay on screen, folded, with the editor still there to type in. */
  async #edited(repo: RepoId, spec: DiffSpec, open: Opened, options: DiffOptions): Promise<FileDiff> {
    const result = await diffEdited(repo, spec, open.path, diffEdit.currentText(), open.shape, options);
    if (!diffEdit.dirty || !SAME_TEXT.includes(result.kind)) return result;
    return unchangedText(open.base, this.diff);
  }

  #edits: ReturnType<typeof setTimeout> | undefined;

  /** The editor's text changed: its diff follows once the typing pauses. */
  edited(): void {
    diffEdit.revision += 1;
    clearTimeout(this.#edits);
    this.#edits = setTimeout(() => void this.reload(), EDIT_SETTLE_MS);
  }

  async expand(repo: RepoId, whole: boolean): Promise<void> {
    if (!this.spec || !this.path) return;
    this.context = expandedContext(this.context ?? DEFAULT_CONTEXT, whole);
    await this.load(repo, this.spec, this.path);
  }

  /** Recomputes this file and nothing else: the panel keeps its scroll and selection. */
  async reload(): Promise<void> {
    if (this.#repo === null || !this.spec || !this.path || this.gone) return;
    await this.load(this.#repo, this.spec, this.path);
  }

  /** Every change to the working tree ends the same way (`runMutation`): the file list,
      the journal behind Undo and the rest read again. App hands its own over once. */
  useMutation(context: MutationContext | null): void {
    this.#mutation = context;
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
    const context = this.#mutation;
    if (context) {
      await runMutation(context, () => discardSelection(lines.repo, lines.request), [lines.request.path], false);
      return;
    }
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
    // A mutation of this very file may have brought it back.
    this.gone = false;
    await this.#rediff();
  }

  /** After a mutation: the shown file is re-diffed when it was one of `paths`; any other
      open diff of the working tree is re-read from disk too (`rereadOthers`), since the
      watcher is quiet right after our own writes and somebody else's edit made meanwhile
      would never reach it (W-06). A pass that re-reads the diff itself passes `false`. */
  afterWrite(paths: readonly string[], rereadOthers = true): Promise<void> {
    if (this.path !== null && paths.includes(this.path)) return this.dropIfAffected(paths);
    return rereadOthers ? this.refreshFromDisk() : Promise.resolve();
  }

  /** The watcher saw the working tree, the index or HEAD move: a diff that reads any of
      them is out of date, and Stage would cut its patch from the old hunks. A diff between
      two commits cannot change. */
  async refreshFromDisk(): Promise<void> {
    const kind = this.spec?.kind;
    if (kind === undefined || kind === "commitVsParent" || kind === "commitVsCommit") return;
    await this.#rediff();
  }

  /** The file open in the editor is read again first: a clean editor follows the disk. */
  async #rediff(): Promise<void> {
    await diffEdit.diskChanged();
    const before = this.#generation;
    await this.reload();
    // Another file was opened meanwhile, or the reload never started: not this pass's to clear.
    if (this.gone || this.#generation !== before + 1) return;
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
    this.gone = false;
  }
}

const WORKING_SPECS: readonly DiffSpec["kind"][] = ["workTreeVsIndex", "indexVsHead", "commitVsWorkTree"];

/** How long the editor's text rests before it is diffed again. */
export const EDIT_SETTLE_MS = 80;

/** Results that say the content is the base's own. */
const SAME_TEXT: readonly FileDiff["kind"][] = ["unchanged", "whitespaceOnly", "eolOnly", "modeOnly", "emptyFile"];

/** The base as a diff of every line unchanged, which the view folds into one strip. The line
    endings and the language are those of the diff it follows (`like`). */
export function unchangedText(base: string, like: FileDiff | null): FileDiff {
  const lines = base === "" ? [] : base.split("\n");
  const rows = lines.map((text, index) => ({ kind: "context" as const, old: index + 1, new: index + 1, text, noNewline: false }));
  const total = rows.length;
  return {
    kind: "text",
    hunks: total === 0 ? [] : [{ oldStart: 1, oldLines: total, newStart: 1, newLines: total, header: `@@ -1,${total} +1,${total} @@`, rows }],
    eol: like?.kind === "text" ? like.eol : { old: "lf", new: "lf", normalized: false },
    lossyEncoding: false,
    language: like?.kind === "text" ? like.language : null,
    oldTotal: total,
    newTotal: total,
    oldText: null,
    newText: null,
    converted: null,
  };
}

function sameSpec(a: DiffSpec | null, b: DiffSpec): boolean {
  if (a === null || a.kind !== b.kind) return false;
  if (a.kind === "commitVsParent" && b.kind === "commitVsParent") return a.oid === b.oid;
  if (a.kind === "commitVsCommit" && b.kind === "commitVsCommit") return a.a === b.a && a.b === b.b;
  return true;
}

export const diff = new DiffStore();
