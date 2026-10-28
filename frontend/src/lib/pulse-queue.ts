import type { RepoPulse } from "$lib/ipc/bindings";

/** Between two servers asked: the server lane is background work and should look like it. */
export const GAP_MS = 300;
/** How often a server lane held back by the repository on screen looks again. */
export const BUSY_POLL_MS = 500;
/** Rows read side by side: each is a few milliseconds of local reads (R-353). */
export const PARALLEL = 4;

export interface PulseDeps {
  pulse(root: string): Promise<RepoPulse>;
  /** `false` when the fetch failed; it never throws for that. */
  fetch(root: string): Promise<boolean>;
  /** The repository on screen is doing something; no server is asked until it is done. */
  busy(): boolean;
  /** The panels own this one and read its full status themselves: no pulse. */
  owned(root: string): boolean;
  sleep(ms: number): Promise<void>;
  onPulse(root: string, pulse: RepoPulse): void;
  onFetch(root: string, ok: boolean): void;
}

/** The status of the rows of the Repositories list, in two lanes (R-353, R-913). Neither starts
    while the repository on screen is busy. Reads — branch,
    changes, ahead/behind of the local tracking ref — run `PARALLEL` at a time, at once: they
    are what the list shows. Servers are asked one at a time, with a pause between two and
    none while the repository on screen is busy; each answer is followed by a read. A request
    for a root already waiting merges with it; `cancel` and `stop` drop what an answer still
    on its way would have written. */
export class PulseQueue {
  readonly #deps: PulseDeps;
  #reads = new Set<string>();
  #fetches = new Set<string>();
  #reading = new Set<string>();
  #dropped = new Set<string>();
  #fetching = false;
  /** The root whose server is being asked now, and whether it was cancelled meanwhile. */
  #asking: string | null = null;
  #askDropped = false;
  #generation = 0;

  constructor(deps: PulseDeps) {
    this.#deps = deps;
  }

  get pending(): string[] {
    return [...new Set([...this.#reads, ...this.#fetches])];
  }

  /** `first` jumps the queue: something happened in that repository just now. `fetch` asks
      its server first; the read follows the answer. */
  request(root: string, { fetch = false, first = false } = {}): void {
    if (fetch) {
      this.#fetches.add(root);
      this.#kickFetches();
      return;
    }
    if (first) this.#reads = new Set([root, ...[...this.#reads].filter((each) => each !== root)]);
    else this.#reads.add(root);
    this.#kickReads();
  }

  cancel(root: string): void {
    this.#reads.delete(root);
    this.#fetches.delete(root);
    if (this.#reading.has(root)) this.#dropped.add(root);
    if (this.#asking === root) this.#askDropped = true;
  }

  stop(): void {
    this.#reads.clear();
    this.#fetches.clear();
    this.#generation += 1;
  }

  #kickReads(): void {
    while (this.#reading.size < PARALLEL) {
      const root = [...this.#reads].find((each) => !this.#reading.has(each));
      if (root === undefined) return;
      this.#reads.delete(root);
      this.#reading.add(root);
      void this.#read(root, this.#generation).finally(() => {
        this.#reading.delete(root);
        this.#dropped.delete(root);
        this.#kickReads();
      });
    }
  }

  async #read(root: string, generation: number): Promise<void> {
    while (this.#deps.busy()) {
      await this.#deps.sleep(BUSY_POLL_MS);
      if (generation !== this.#generation || this.#dropped.has(root)) return;
    }
    if (this.#deps.owned(root)) return;
    const read = await this.#deps.pulse(root).catch(() => null);
    if (generation === this.#generation && read && !this.#dropped.has(root)) this.#deps.onPulse(root, read);
  }

  #kickFetches(): void {
    if (this.#fetching) return;
    this.#fetching = true;
    void this.#drainFetches().finally(() => {
      this.#fetching = false;
      if (this.#fetches.size > 0) this.#kickFetches();
    });
  }

  async #drainFetches(): Promise<void> {
    const generation = this.#generation;
    const live = () => generation === this.#generation;
    while (this.#fetches.size > 0 && live()) {
      while (this.#deps.busy()) {
        await this.#deps.sleep(BUSY_POLL_MS);
        if (!live()) return;
      }
      const root = this.#fetches.values().next().value;
      if (root === undefined) return;
      this.#fetches.delete(root);
      this.#asking = root;
      this.#askDropped = false;
      const ok = await this.#deps.fetch(root).catch(() => false);
      this.#asking = null;
      if (!live()) return;
      if (this.#askDropped) continue;
      this.#deps.onFetch(root, ok);
      this.request(root, { first: true });
      await this.#deps.sleep(GAP_MS);
    }
  }
}
