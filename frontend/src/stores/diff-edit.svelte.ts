import type { DiffSpec, RepoId } from "$lib/ipc";
import { readEditable, saveEditable, type Shape } from "$lib/ipc/editable";
import { confirmation } from "$stores/confirm.svelte";

export interface Opened {
  repo: RepoId;
  spec: DiffSpec;
  path: string;
  base: string;
  /** What the editor starts from, and what Reload brings back. */
  text: string;
  shape: Shape;
  stamp: string;
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

  get active(): boolean {
    return this.opened !== null || this.refused !== null || this.loading;
  }

  /** The file Edit was pressed on, from the press until `stop`. */
  target = $state<string | null>(null);

  isFor(path: string | null): boolean {
    return path !== null && this.target === path;
  }

  async start(repo: RepoId, spec: DiffSpec, path: string): Promise<void> {
    this.stop();
    this.target = path;
    this.loading = true;
    try {
      const file = await readEditable(repo, spec, path);
      if (this.target !== path) return;
      if (file.kind === "refused") this.refused = file.reason;
      else this.opened = { repo, spec, path, base: file.base, text: file.text, shape: file.shape, stamp: file.stamp };
    } catch (err) {
      this.refused = String(err instanceof Error ? err.message : err);
    } finally {
      if (this.target === path) this.loading = false;
    }
  }

  /** The editor's text, set by the pane while it is mounted and kept when it goes. */
  #text: () => string = () => this.opened?.text ?? "";

  #pane: { focused: () => boolean; save: () => void } | null = null;

  attach(text: () => string, pane: { focused: () => boolean; save: () => void }): void {
    this.#text = text;
    this.#pane = pane;
  }

  /** The menu took Ctrl+S (Stash All) before the page: while the editor has the focus, it
      is Save. True when it was taken here. */
  saveShortcut(): boolean {
    if (!this.#pane?.focused()) return false;
    this.#pane.save();
    return true;
  }

  /** The pane is leaving the screen: freeze what it held, so a later save still has it. */
  detach(): void {
    const last = this.#text();
    this.#text = () => last;
    this.#pane = null;
  }

  currentText(): string {
    return this.#text();
  }

  /** Before the editor goes (Done, another file, another repository): unsaved edits are
      saved or dropped on purpose, never silently. False when the save failed. */
  async leave(): Promise<boolean> {
    if (this.dirty && this.opened) {
      const save = await confirmation.ask({
        title: "Unsaved edits",
        message: `Save your edits to ${this.opened.path}? Not saving discards them.`,
        confirm: "Save",
      });
      if (save && !(await this.save(this.#text()))) return false;
    }
    this.stop();
    return true;
  }

  stop(): void {
    this.target = null;
    this.opened = null;
    this.refused = null;
    this.loading = false;
    this.dirty = false;
    this.changedOnDisk = false;
    this.error = null;
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
      this.opened = { ...opened, text, stamp: outcome.stamp };
      this.dirty = false;
      this.changedOnDisk = false;
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
    this.opened = { ...opened, base, text, shape, stamp };
    this.dirty = false;
    this.changedOnDisk = false;
    this.generation += 1;
  }
}

export const diffEdit = new DiffEditStore();
