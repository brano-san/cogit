import {
  CogitError,
  diffFile,
  imageSides,
  type DiffSpec,
  type FileDiff,
  type Hunk,
  type Whitespace,
  type RepoId,
} from "$lib/ipc";
import { expandedContext } from "$lib/diff-rows";
import { settings } from "./settings.svelte";

const DEFAULT_CONTEXT = 3;

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

  get hunks(): Hunk[] {
    return this.diff?.kind === "text" ? this.diff.hunks : [];
  }

  get stageable(): boolean {
    return this.spec?.kind === "workTreeVsIndex" || this.spec?.kind === "indexVsHead";
  }

  /** Clicking down the file list outruns the backend; stale diffs lose. */
  #generation = 0;

  async load(repo: RepoId, spec: DiffSpec, path: string): Promise<void> {
    const generation = ++this.#generation;
    if (path !== this.path) this.context = null;
    this.path = path;
    this.spec = spec;
    this.error = null;
    this.loading = true;

    try {
      const result = await diffFile(repo, spec, path, {
        ...settings.diffOptions,
        ...(this.context === null ? {} : { contextLines: this.context }),
        ignoreWhitespace: this.whitespace,
      });
      if (generation !== this.#generation) return;
      this.diff = result;
      this.images = result.kind === "image" ? await imageSides(repo, spec, path) : [null, null];
    } catch (err) {
      if (generation !== this.#generation) return;
      this.diff = null;
      this.error =
        err instanceof CogitError ? err : new CogitError({ kind: "internal", data: String(err) });
    } finally {
      if (generation === this.#generation) this.loading = false;
    }
  }

  async expand(repo: RepoId, whole: boolean): Promise<void> {
    if (!this.spec || !this.path) return;
    this.context = expandedContext(this.context ?? DEFAULT_CONTEXT, whole);
    await this.load(repo, this.spec, this.path);
  }

  dropIfAffected(paths: readonly string[]): void {
    if (this.path !== null && paths.includes(this.path)) this.clear();
  }

  /** Re-runs the last diff with the new mode so the panel does not go stale. */
  async setWhitespace(repo: RepoId, mode: Whitespace): Promise<void> {
    this.whitespace = mode;
    if (this.spec && this.path) await this.load(repo, this.spec, this.path);
  }

  clear(): void {
    this.#generation += 1;
    this.path = null;
    this.spec = null;
    this.context = null;
    this.diff = null;
    this.images = [null, null];
    this.loading = false;
    this.error = null;
  }
}

export const diff = new DiffStore();
