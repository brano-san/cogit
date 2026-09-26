import { pullProbe, repoPulse as readPulse, type RepoPulse } from "$lib/ipc/repo-rows";
import { PulseQueue } from "$lib/pulse-queue";

/** A server that has not answered in this long stops holding the queue; the process runs
    on, its answer is not listened to, and its root is not asked again until it ends. */
const PROBE_TIMEOUT_MS = 120_000;
/** Past the 150 ms of quiet that ends a switch or a close, so the read is not part of it. */
const LEFT_READ_DELAY_MS = 500;
const REVISIT_MS = 60_000;

function within<T>(work: Promise<T>, ms: number, fallback: T): Promise<T> {
  return new Promise((resolve) => {
    const timer = setTimeout(() => resolve(fallback), ms);
    void work.then(
      (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      () => {
        clearTimeout(timer);
        resolve(fallback);
      },
    );
  });
}

/** Indicators of the rows of the Repositories list that the panels do not show, open or
    closed, kept fresh in the background (R-353). */
class RepoPulseStore {
  pulses = $state.raw<ReadonlyMap<string, RepoPulse>>(new Map());
  /** Roots whose last probe of the server failed: whether there is anything to pull is unknown. */
  unknown = $state.raw<ReadonlySet<string>>(new Set());
  /** The server has commits HEAD lacks, whatever the local tracking ref says (R-354). */
  remoteAhead = $state.raw<ReadonlySet<string>>(new Set());

  #busy: () => boolean = () => false;
  #owned: string | null = null;
  #seen = new Set<string>();
  #roots: readonly string[] = [];
  #timer: ReturnType<typeof setInterval> | null = null;
  #every = 0;
  #revisited = -Infinity;
  /** Roots whose probe has not ended, answered in time or not (R-482). */
  #probing = new Set<string>();

  readonly #queue = new PulseQueue({
    pulse: readPulse,
    fetch: (root) => this.#probe(root),
    busy: () => this.#busy(),
    owned: (root) => root === this.#owned,
    sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
    onPulse: (root, pulse) => {
      // Read before the panels took it over; they keep it current now.
      if (root === this.#owned) return;
      this.pulses = new Map([...this.pulses, [root, pulse]]);
    },
    onFetch: (root, ok) => {
      if (ok === this.unknown.has(root)) {
        const next = new Set(this.unknown);
        if (ok) next.delete(root);
        else next.add(root);
        this.unknown = next;
      }
    },
  });

  /** What counts as the repository on screen being busy: nothing starts while it is. */
  setBusy(busy: () => boolean): void {
    this.#busy = busy;
  }

  /** The row the panels show reads its full status itself. The one they leave has had
      no pulse while it was on screen, so it gets one — once the switch or close that left
      it has settled, not inside it: the read cost `repo.close` 3–7 ms. */
  setOwned(root: string | null): void {
    const left = this.#owned;
    this.#owned = root;
    // Kept, it would outlive what the panels do there and show once the row is let go.
    if (root !== null && this.pulses.has(root)) {
      const next = new Map(this.pulses);
      next.delete(root);
      this.pulses = next;
    }
    if (left === null || left === root || !this.#roots.includes(left)) return;
    setTimeout(() => {
      if (left !== this.#owned && this.#roots.includes(left)) this.changed(left);
    }, LEFT_READ_DELAY_MS);
  }

  /** Every row of the list; a row never read is read once. */
  watch(roots: readonly string[]): void {
    this.#roots = roots;
    for (const root of roots) {
      if (this.#seen.has(root)) continue;
      this.#seen.add(root);
      this.#queue.request(root);
    }
  }

  /** The window came back into focus. Whatever was done meanwhile in a closed repository
      has no watcher to report it, so every row the panels do not own is read again — at
      most once a minute, through the same queue (F-451). */
  revisit(): void {
    const now = Date.now();
    if (now - this.#revisited < REVISIT_MS) return;
    this.#revisited = now;
    for (const root of this.#roots) {
      if (root !== this.#owned) this.#queue.request(root);
    }
  }

  /** Something happened in it just now. */
  changed(root: string): void {
    this.#queue.request(root, { first: true });
  }

  /** Its refs moved (a fetch, a pull): the tracking ref speaks for the server again. */
  refsMoved(root: string): void {
    this.#setAhead(root, false);
  }

  /** The server is asked, nothing is fetched: `ls-remote` writes no ref (R-354). `false`
      while the one before is still out: whether there is anything to pull is still unknown. */
  #probe(root: string): Promise<boolean> {
    if (this.#probing.has(root)) return Promise.resolve(false);
    this.#probing.add(root);
    let waiting = true;
    const asked = pullProbe(root)
      .then((ahead) => {
        // Given up on, removed, or the check turned off while the server took its time.
        if (waiting && this.#seen.has(root) && this.#every > 0) this.#setAhead(root, ahead === true);
        return true;
      })
      .finally(() => this.#probing.delete(root));
    return within(asked, PROBE_TIMEOUT_MS, false).then((ok) => {
      waiting = false;
      return ok;
    });
  }

  #setAhead(root: string, ahead: boolean): void {
    if (this.remoteAhead.has(root) === ahead) return;
    const next = new Set(this.remoteAhead);
    if (ahead) next.add(root);
    else next.delete(root);
    this.remoteAhead = next;
  }

  forget(root: string): void {
    this.#queue.cancel(root);
    this.#seen.delete(root);
    this.#setAhead(root, false);
    if (this.unknown.has(root)) {
      const next = new Set(this.unknown);
      next.delete(root);
      this.unknown = next;
    }
    if (!this.pulses.has(root)) return;
    const next = new Map(this.pulses);
    next.delete(root);
    this.pulses = next;
  }

  /** `0` stops the background check. Every tick asks each row's server once, in list order. */
  fetchEvery(minutes: number): void {
    if (minutes === this.#every) return;
    this.#every = minutes;
    if (this.#timer !== null) clearInterval(this.#timer);
    this.#timer = null;
    if (minutes <= 0) {
      // What the servers said is no longer being kept current.
      this.unknown = new Set();
      this.remoteAhead = new Set();
      return;
    }
    this.#timer = setInterval(
      () => {
        for (const root of this.#roots) this.#queue.request(root, { fetch: true });
      },
      minutes * 60_000,
    );
  }
}

export const repoPulse = new RepoPulseStore();
