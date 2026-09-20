import { CogitError, scanForRepositories, type ScanHit } from "$lib/ipc";

class ScanStore {
  folder = $state<string | null>(null);
  hits = $state.raw<ScanHit[]>([]);
  chosen = $state.raw<ReadonlySet<string>>(new Set());
  busy = $state(false);
  done = $state(false);
  error = $state<CogitError | null>(null);

  /** A second scan started from the dialog must not be joined by the first one's hits. */
  #generation = 0;

  get openable(): ScanHit[] {
    return this.hits.filter((hit) => !hit.alreadyOpen);
  }

  get selected(): string[] {
    return this.openable.filter((hit) => this.chosen.has(hit.root)).map((hit) => hit.root);
  }

  async run(folder: string, depth: number): Promise<void> {
    const generation = ++this.#generation;
    this.folder = folder;
    this.hits = [];
    this.chosen = new Set();
    this.error = null;
    this.done = false;
    this.busy = true;

    try {
      await scanForRepositories(folder, depth, (hit) => {
        if (generation !== this.#generation) return;
        this.hits = [...this.hits, hit];
        if (!hit.alreadyOpen) this.chosen = new Set([...this.chosen, hit.root]);
      });
      if (generation === this.#generation) this.done = true;
    } catch (err) {
      if (generation !== this.#generation) return;
      this.error =
        err instanceof CogitError ? err : new CogitError({ kind: "internal", data: String(err) });
    } finally {
      if (generation === this.#generation) this.busy = false;
    }
  }

  toggle(root: string): void {
    if (this.hits.some((hit) => hit.root === root && hit.alreadyOpen)) return;
    const next = new Set(this.chosen);
    if (!next.delete(root)) next.add(root);
    this.chosen = next;
  }

  toggleAll(): void {
    const all = this.openable.map((hit) => hit.root);
    this.chosen = this.chosen.size === all.length ? new Set() : new Set(all);
  }

  clear(): void {
    this.#generation += 1;
    this.folder = null;
    this.hits = [];
    this.chosen = new Set();
    this.busy = false;
    this.done = false;
    this.error = null;
  }
}

export const scan = new ScanStore();
