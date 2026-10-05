import type { DiffSpec, RepoId } from "$lib/ipc";
import { readEditable, saveEditable, type Shape } from "$lib/ipc/editable";
import { unsavedPrompt } from "$stores/unsaved-prompt.svelte";

/** `diff`: the base read-only beside the file (2-way); `single`: the file alone (Edit). */
export type EditMode = "diff" | "single";

/** Where the caret goes when the editor opens: a click in the diff's right pane. */
export interface CaretAt {
  line: number;
  column: number;
}

export interface Opened {
  repo: RepoId;
  spec: DiffSpec;
  path: string;
  mode: EditMode;
  base: string;
  /** What the editor starts from, and what Reload brings back. */
  text: string;
  shape: Shape;
  stamp: string;
  at: CaretAt | null;
}

interface Pane {
  text: () => string;
  focused: () => boolean;
  save: () => void;
}

/** The working file being edited in the Diff panel. The editor holds the text; this holds
    where it came from, whether it is dirty, and whether the file moved on under it. */
class DiffEditStore {
  opened = $state.raw<Opened | null>(null);
  /** `Edit` was pressed but the file cannot round-trip: why, in the panel. */
  refused = $state<string | null>(null);
  loading = $state(false);
  dirty = $state(false);
  /** Someone else wrote the file while it was edited: Reload or Keep Mine. */
  changedOnDisk = $state(false);
  saving = $state(false);
  error = $state<string | null>(null);
  /** Bumped when the editor must take `opened.text` again (a reload). */
  generation = $state(0);
  /** The file Edit was asked for, from then until `stop`. */
  target = $state<string | null>(null);
  /** After a save: the repository status and the Files list are read again (App sets it). */
  onSaved: ((path: string) => void) | null = null;

  #pane: Pane | null = null;
  #last = "";

  get active(): boolean {
    return this.opened !== null || this.refused !== null || this.loading;
  }

  isFor(path: string | null): boolean {
    return path !== null && this.target === path;
  }

  async start(repo: RepoId, spec: DiffSpec, path: string, mode: EditMode = "diff", at: CaretAt | null = null): Promise<void> {
    this.stop();
    this.target = path;
    this.loading = true;
    try {
      const file = await readEditable(repo, spec, path);
      if (this.target !== path) return;
      if (file.kind === "refused") this.refused = file.reason;
      else this.opened = { repo, spec, path, mode, base: file.base, text: file.text, shape: file.shape, stamp: file.stamp, at };
    } catch (err) {
      this.refused = String(err instanceof Error ? err.message : err);
    } finally {
      if (this.target === path) this.loading = false;
    }
  }

  /** The editor's text, read from the pane while it is mounted and kept when it goes. */
  attach(pane: Pane): void {
    this.#pane = pane;
  }

  detach(): void {
    this.#last = this.#pane?.text() ?? this.#last;
    this.#pane = null;
  }

  currentText(): string {
    return this.#pane?.text() ?? (this.#last || this.opened?.text || "");
  }

  /** The menu took Ctrl+S (Stash All) before the page: while the editor has the focus, it
      is Save. True when it was taken here. */
  saveShortcut(): boolean {
    if (!this.#pane?.focused()) return false;
    this.#pane.save();
    return true;
  }

  /** Before the editor goes: unsaved edits are saved or dropped on purpose, never silently.
      False when the user stays (Cancel) or the save failed; the editor is still open then.
      Not \`cancellable\` when it goes whatever the answer (the repository changed). */
  async leave(cancellable = true): Promise<boolean> {
    const opened = this.opened;
    if (this.dirty && opened) {
      const choice = await unsavedPrompt.ask("Unsaved Edits", opened.path, cancellable);
      if (choice === "cancel") return false;
      if (choice === "save" && !(await this.save(this.currentText()))) return false;
    }
    this.stop();
    return true;
  }

  /** The Diff panel is about to show `path` under `spec`: the editor of another file goes
      first, or the switch does not happen (Cancel). */
  async leaveFor(path: string, spec: DiffSpec): Promise<boolean> {
    const opened = this.opened;
    if (this.target === null) return true;
    if (opened && opened.path === path && opened.spec.kind === spec.kind) return true;
    return this.leave();
  }

  stop(): void {
    this.target = null;
    this.opened = null;
    this.refused = null;
    this.loading = false;
    this.dirty = false;
    this.changedOnDisk = false;
    this.error = null;
    this.#last = "";
  }

  /** `force`: Keep Mine, over whatever is on disk now. True when the file is written. */
  async save(text: string, force = false): Promise<boolean> {
    const opened = this.opened;
    if (!opened || this.saving) return false;
    this.saving = true;
    this.error = null;
    try {
      const outcome = await saveEditable(opened.repo, opened.path, text, opened.shape, opened.stamp, force);
      if (outcome.kind === "changedOnDisk") {
        this.changedOnDisk = true;
        return false;
      }
      this.opened = { ...opened, text, stamp: outcome.stamp, at: null };
      this.dirty = false;
      this.changedOnDisk = false;
      this.onSaved?.(opened.path);
      return true;
    } catch (err) {
      this.error = String(err instanceof Error ? err.message : err);
      return false;
    } finally {
      this.saving = false;
    }
  }

  /** The watcher saw the file change: our own save leaves the stamp as it is. A clean
      editor follows the disk; a dirty one asks. */
  async diskChanged(): Promise<void> {
    const opened = this.opened;
    if (!opened || this.saving) return;
    const file = await readEditable(opened.repo, opened.spec, opened.path).catch(() => null);
    if (!file || file.kind !== "text" || this.opened !== opened || file.stamp === opened.stamp) return;
    if (this.dirty) {
      this.changedOnDisk = true;
      return;
    }
    this.#take(file.base, file.text, file.shape, file.stamp);
  }

  /** Reload: the disk wins, the edits go (undo brings them back). */
  async reload(): Promise<void> {
    const opened = this.opened;
    if (!opened) return;
    const file = await readEditable(opened.repo, opened.spec, opened.path).catch(() => null);
    if (!file || this.opened !== opened) return;
    if (file.kind === "refused") {
      this.opened = null;
      this.refused = file.reason;
      return;
    }
    this.#take(file.base, file.text, file.shape, file.stamp);
  }

  #take(base: string, text: string, shape: Shape, stamp: string): void {
    const opened = this.opened;
    if (!opened) return;
    this.opened = { ...opened, base, text, shape, stamp, at: null };
    this.dirty = false;
    this.changedOnDisk = false;
    this.generation += 1;
  }
}

export const diffEdit = new DiffEditStore();
