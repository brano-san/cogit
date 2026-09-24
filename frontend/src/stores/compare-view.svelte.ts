import { type CogitError, type DiffSpec, type FileEntry, type RepoId, toCogitError } from "$lib/ipc";
import { compareFiles } from "$lib/ipc/ref-ops";
import { diff } from "$stores/diff.svelte";

/** Compare with HEAD / with Selected Commit: the Files panel lists what differs between
    two commits while the graph keeps `to` selected and `from` marked (#33–#35). */
class CompareViewStore {
  repo = $state.raw<RepoId | null>(null);
  from = $state<string | null>(null);
  to = $state<string | null>(null);
  files = $state.raw<FileEntry[]>([]);
  loading = $state(false);
  error = $state<CogitError | null>(null);

  #generation = 0;

  async show(repo: RepoId, from: string, to: string): Promise<void> {
    const generation = ++this.#generation;
    this.repo = repo;
    this.from = from;
    this.to = to;
    this.files = [];
    this.error = null;
    this.loading = true;
    try {
      const found = await compareFiles(repo, from, to);
      if (generation === this.#generation) this.files = found;
    } catch (err) {
      if (generation !== this.#generation) return;
      this.error = toCogitError(err);
    } finally {
      if (generation === this.#generation) this.loading = false;
    }
  }

  /** Only while the graph still has the compared commit selected. */
  showing(selected: string | null): boolean {
    return this.to !== null && this.to === selected;
  }

  get spec(): DiffSpec | null {
    return this.from && this.to ? { kind: "commitVsCommit", a: this.from, b: this.to } : null;
  }

  /** A second click on the open file closes it, as in every other list of the panel. */
  open(path: string): void {
    const spec = this.spec;
    if (!spec || this.repo === null) return;
    if (diff.path === path) {
      diff.clear();
      return;
    }
    void diff.load(this.repo, spec, path);
  }

  clear(): void {
    this.#generation += 1;
    this.repo = null;
    this.from = null;
    this.to = null;
    this.files = [];
    this.loading = false;
    this.error = null;
  }
}

export const compareView = new CompareViewStore();
