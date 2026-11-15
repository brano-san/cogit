import { pathKey } from "$lib/welcome";
import { cancelOperation, type CogitError, scanForRepositories, type ScanHit, toCogitError } from "$lib/ipc";

/** What the dialog names as the scanned place when no folder was chosen. */
export const ALL_DRIVES = "All drives";

class ScanStore {
  /** The folder scanned, or `ALL_DRIVES`; null before any scan. */
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
  #pending: ScanHit[] = [];
  #publish: ReturnType<typeof setTimeout> | null = null;

  #flushHits(): void {
    if (this.#publish !== null) clearTimeout(this.#publish);
    this.#publish = null;
    if (this.#pending.length === 0) return;
    this.hits = this.hits.concat(this.#pending);
    this.#pending = [];
  }

  #discardHits(): void {
    if (this.#publish !== null) clearTimeout(this.#publish);
    this.#publish = null;
    this.#pending = [];
  }

  get openable(): ScanHit[] {
    return this.hits.filter((hit) => !hit.alreadyOpen);
  }

  get selected(): string[] {
    return this.openable.filter((hit) => this.chosen.has(hit.root)).map((hit) => hit.root);
  }

  /** `folder` null: every drive. */
  async run(folder: string | null, depth: number, listed: readonly string[] = []): Promise<void> {
    const known = new Set(listed.map(pathKey));
    this.#stopWalk();
    this.#discardHits();
    const generation = ++this.#generation;
    this.folder = folder ?? ALL_DRIVES;
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
          // Nothing ticked until the user ticks it: a search of a whole drive found dozens
          // and one click opened them all (R-821).
          this.#pending.push({ ...hit, alreadyOpen: hit.alreadyOpen || known.has(pathKey(hit.root)) });
          // Bound both live-result latency and copies of the accumulated list. Every
          // publication is a new snapshot, so old subscribers cannot see mutations.
          if (this.#pending.length >= 128) this.#flushHits();
          else this.#publish ??= setTimeout(() => this.#flushHits(), 16);
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
        this.#flushHits();
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
    const visible = shown === null ? null : new Set(shown);
    const rows: string[] = [];
    for (const hit of this.hits) {
      if (!hit.alreadyOpen && (visible === null || visible.has(hit.root))) rows.push(hit.root);
    }
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
    this.#discardHits();
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
