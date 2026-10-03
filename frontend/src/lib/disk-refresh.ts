import type { ChangeKind } from "./ipc";

export type DiskPass = (kinds: ReadonlySet<ChangeKind>, arrived: () => ReadonlySet<ChangeKind>) => Promise<void>;

/** Watcher events, answered one refresh at a time. A burst is collected until it has been
    quiet for `settleMs`, but never held back longer than `maxWaitMs` since its first
    event, or a stream that never pauses would not be answered at all; a burst that settles while a pass still reads waits for it and
    then gets its own pass, so a folder written without pause (`.vs/`, `.svelte-kit/`)
    never has two full refreshes reading at once (R-144). `arrived` tells the pass what
    came after it began: the panels those events marked stale stay marked. */
export class DiskPasses {
  readonly #settleMs: number;
  readonly #maxWaitMs: number;
  readonly #run: DiskPass;
  readonly #report: (err: unknown) => void;
  #since: number | undefined;
  #pending = new Set<ChangeKind>();
  #arrived = new Set<ChangeKind>();
  #timer: ReturnType<typeof setTimeout> | undefined;
  #running = false;
  #again = false;

  constructor(settleMs: number, maxWaitMs: number, run: DiskPass, report: (err: unknown) => void) {
    this.#settleMs = settleMs;
    this.#maxWaitMs = maxWaitMs;
    this.#run = run;
    this.#report = report;
  }

  add(kind: ChangeKind): void {
    const now = Date.now();
    this.#since ??= now;
    this.#pending.add(kind);
    if (this.#running) this.#arrived.add(kind);
    clearTimeout(this.#timer);
    this.#timer = setTimeout(
      () => this.#settled(),
      Math.max(0, Math.min(this.#settleMs, this.#since + this.#maxWaitMs - now)),
    );
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
        this.#since = undefined;
        const arrived = new Set<ChangeKind>();
        this.#arrived = arrived;
        // A pass that fails is reported and its kinds are not asked for again, or a broken
        // config would loop; what arrived meanwhile still gets its pass.
        if (kinds.size > 0) {
          try {
            await this.#run(kinds, () => arrived);
          } catch (err) {
            this.#report(err);
          }
        }
      } while (this.#again);
    } finally {
      this.#running = false;
    }
  }
}
