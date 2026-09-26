import { cancelOperation, type CogitError, scanForRepositories, type ScanHit, toCogitError } from "$lib/ipc";

class ScanStore {
  folder = $state<string | null>(null);
  hits = $state.raw<ScanHit[]>([]);
  chosen = $state.raw<ReadonlySet<string>>(new Set());
  busy = $state(false);
  done = $state(false);
  error = $state<CogitError | null>(null);

  /** A second scan started from the dialog must not be joined by the first one's hits. */
  #generation = 0;
  /** The walk still going, by the id that stops it. */
  #walking: number | null = null;

  get openable(): ScanHit[] {
    return this.hits.filter((hit) => !hit.alreadyOpen);
  }

  get selected(): string[] {
    return this.openable.filter((hit) => this.chosen.has(hit.root)).map((hit) => hit.root);
  }

  async run(folder: string, depth: number): Promise<void> {
    this.#stopWalk();
    const generation = ++this.#generation;
    this.folder = folder;
    this.hits = [];
    this.chosen = new Set();
    this.error = null;
    this.done = false;
    this.busy = true;

    try {
      await scanForRepositories(
        folder,
        depth,
        (hit) => {
          if (generation !== this.#generation) return;
          this.hits = [...this.hits, hit];
          if (!hit.alreadyOpen) this.chosen = new Set([...this.chosen, hit.root]);
        },
        (id) => {
          if (generation === this.#generation) this.#walking = id;
          else void cancelOperation(id);
        },
      );
      if (generation === this.#generation) this.done = true;
    } catch (err) {
      if (generation !== this.#generation) return;
      this.error = toCogitError(err);
    } finally {
      if (generation === this.#generation) {
        this.busy = false;
        this.#walking = null;
      }
    }
  }

  #stopWalk(): void {
    if (this.#walking !== null) void cancelOperation(this.#walking);
    this.#walking = null;
  }

  toggle(root: string): void {
    if (this.hits.some((hit) => hit.root === root && hit.alreadyOpen)) return;
    const next = new Set(this.chosen);
    if (!next.delete(root)) next.add(root);
    this.chosen = next;
  }

  /** Select All and Select None act on `shown`, the rows a filter leaves; what it hides
      keeps its tick either way. */
  toggleAll(shown: readonly string[] | null = null): void {
    const rows = this.openable.map((hit) => hit.root).filter((root) => shown === null || shown.includes(root));
    const next = new Set(this.chosen);
    const all = rows.every((root) => next.has(root));
    for (const root of rows) {
      if (all) next.delete(root);
      else next.add(root);
    }
    this.chosen = next;
  }

  clear(): void {
    this.#stopWalk();
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
