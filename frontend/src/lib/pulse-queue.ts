import type { RepoPulse } from "$lib/ipc/bindings";

/** Between two repositories: the queue is background work and should look like it. */
export const GAP_MS = 300;
/** How often a queue held back by the repository on screen looks again. */
export const BUSY_POLL_MS = 500;

export interface PulseDeps {
  pulse(root: string): Promise<RepoPulse>;
  /** `false` when the fetch failed; it never throws for that. */
  fetch(root: string): Promise<boolean>;
  /** The repository on screen is doing something; nothing starts until it is done. */
  busy(): boolean;
  /** The panels own this one and read its full status themselves: no pulse. */
  owned(root: string): boolean;
  sleep(ms: number): Promise<void>;
  onPulse(root: string, pulse: RepoPulse): void;
  onFetch(root: string, ok: boolean): void;
}

interface Job {
  fetch: boolean;
}

/** The status of the rows of the Repositories list, one repository at a time, with a pause
    between two and none at all while the repository on screen is busy (R-353). A request
    for a root already waiting merges with it; `cancel` and `stop` drop what an answer
    still on its way would have written. */
export class PulseQueue {
  readonly #deps: PulseDeps;
  #jobs = new Map<string, Job>();
  #running = false;
  #generation = 0;
  #current: string | null = null;
  #dropped = false;

  constructor(deps: PulseDeps) {
    this.#deps = deps;
  }

  get pending(): string[] {
    return [...this.#jobs.keys()];
  }

  /** `first` jumps the queue: something happened in that repository just now. */
  request(root: string, { fetch = false, first = false } = {}): void {
    const job = { fetch: fetch || (this.#jobs.get(root)?.fetch ?? false) };
    if (first) {
      this.#jobs.delete(root);
      this.#jobs = new Map([[root, job], ...this.#jobs]);
    } else {
      this.#jobs.set(root, job);
    }
    this.#kick();
  }

  cancel(root: string): void {
    this.#jobs.delete(root);
    if (this.#current === root) this.#dropped = true;
  }

  stop(): void {
    this.#jobs.clear();
    this.#generation += 1;
  }

  #kick(): void {
    if (this.#running) return;
    this.#running = true;
    void this.#drain().finally(() => {
      this.#running = false;
      if (this.#jobs.size > 0) this.#kick();
    });
  }

  async #drain(): Promise<void> {
    const generation = this.#generation;
    const live = () => generation === this.#generation;
    while (this.#jobs.size > 0 && live()) {
      while (this.#deps.busy()) {
        await this.#deps.sleep(BUSY_POLL_MS);
        if (!live()) return;
      }
      const next = this.#jobs.entries().next();
      if (next.done) return;
      const [root, job] = next.value;
      this.#jobs.delete(root);
      this.#current = root;
      this.#dropped = false;
      try {
        if (job.fetch) {
          const ok = await this.#deps.fetch(root).catch(() => false);
          if (!live()) return;
          if (!this.#dropped) this.#deps.onFetch(root, ok);
        }
        if (!this.#deps.owned(root)) {
          const read = await this.#deps.pulse(root).catch(() => null);
          if (!live()) return;
          if (read && !this.#dropped) this.#deps.onPulse(root, read);
        }
      } finally {
        this.#current = null;
      }
      await this.#deps.sleep(GAP_MS);
    }
  }
}
