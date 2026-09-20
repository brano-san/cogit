import {
  CogitError,
  DEFAULT_DIFF_OPTIONS,
  diffFile,
  imageSides,
  type DiffSpec,
  type FileDiff,
  type Hunk,
  type Whitespace,
  type RepoId,
} from "$lib/ipc";

class DiffStore {
  path = $state<string | null>(null);
  diff = $state.raw<FileDiff | null>(null);
  loading = $state(false);
  error = $state<CogitError | null>(null);
  spec = $state.raw<DiffSpec | null>(null);
  /** Remembered per repository: the mode outlives switching between files. */
  whitespace = $state<Whitespace>("none");
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
    this.path = path;
    this.spec = spec;
    this.error = null;
    this.loading = true;

    try {
      const result = await diffFile(repo, spec, path, {
        ...DEFAULT_DIFF_OPTIONS,
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
    this.diff = null;
    this.images = [null, null];
    this.loading = false;
    this.error = null;
  }
}

export const diff = new DiffStore();
