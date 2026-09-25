import type { ChangeKind } from "./ipc";

export type DiskPass = (kinds: ReadonlySet<ChangeKind>, arrived: () => ReadonlySet<ChangeKind>) => Promise<void>;

/** Watcher events, answered one refresh at a time. A burst is collected until it has been
    quiet for `settleMs`; a burst that settles while a pass still reads waits for it and
    then gets its own pass, so a folder written without pause (`.vs/`, `.svelte-kit/`)
    never has two full refreshes reading at once (R-144). `arrived` tells the pass what
    came after it began: the panels those events marked stale stay marked. */
export class DiskPasses {
  readonly #settleMs: number;
  readonly #run: DiskPass;
  #pending = new Set<ChangeKind>();
  #arrived = new Set<ChangeKind>();
  #timer: ReturnType<typeof setTimeout> | undefined;
  #running = false;
  #again = false;

  constructor(settleMs: number, run: DiskPass) {
    this.#settleMs = settleMs;
    this.#run = run;
  }

  add(kind: ChangeKind): void {
    this.#pending.add(kind);
    if (this.#running) this.#arrived.add(kind);
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => this.#settled(), this.#settleMs);
  }

  #settled(): void {
    if (this.#running) {
      this.#again = true;
      return;
    }
    void this.#drain();
  }

  async #drain(): Promise<void> {
    this.#running = true;
    try {
      do {
        this.#again = false;
        const kinds = this.#pending;
        this.#pending = new Set();
        const arrived = new Set<ChangeKind>();
        this.#arrived = arrived;
        if (kinds.size > 0) await this.#run(kinds, () => arrived);
      } while (this.#again);
    } finally {
      this.#running = false;
    }
  }
}
