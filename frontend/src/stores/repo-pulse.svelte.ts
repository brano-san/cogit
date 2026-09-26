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
  #ownedMarks: RepoPulse | null = null;
  #seen = new Set<string>();
  #roots: readonly string[] = [];
  #timer: ReturnType<typeof setInterval> | null = null;
  #every = 0;
  #revisited = -Infinity;
  /** Roots whose probe has not ended, answered in time or not (R-482). */
  #probing = new Set<string>();
  #revisitLater: ReturnType<typeof setTimeout> | null = null;

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

  /** The row the panels show reads its full status itself; `marks` is what they show of it
      now. The one they leave has had no pulse while it was on screen, so it gets one — once
      the switch or close that left it has settled, not inside it: the read cost
      `repo.close` 3–7 ms. Until then it keeps the marks the panels last showed (R-542). */
  setOwned(root: string | null, marks: RepoPulse | null = null): void {
    const left = this.#owned;
    const leftMarks = this.#ownedMarks;
    this.#owned = root;
    this.#ownedMarks = marks;
    // Kept, it would outlive what the panels do there and show once the row is let go.
    if (root !== null && this.pulses.has(root)) {
      const next = new Map(this.pulses);
      next.delete(root);
      this.pulses = next;
    }
    if (left === null || left === root || !this.#roots.includes(left)) return;
    if (leftMarks) this.pulses = new Map([...this.pulses, [left, leftMarks]]);
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

  /** The window came back into focus. Whatever was done meanwhile in a repository the
      panels do not show has no watcher to report it (R-351), so every such row is read
      again — at most once a minute, through the same queue (F-451). A focus change inside
      the minute is answered when it is over: dropped, a commit made in a terminal waited
      for the next focus change after it. */
  revisit(): void {
    const now = Date.now();
    const wait = this.#revisited + REVISIT_MS - now;
    if (wait > 0) {
      this.#revisitLater ??= setTimeout(() => {
        this.#revisitLater = null;
        this.revisit();
      }, wait);
      return;
    }
    this.#revisited = now;
    for (const root of this.#roots) {
      if (root !== this.#owned) this.#queue.request(root);
    }
  }

  /** Something happened in it just now. */
  changed(root: string): void {
    this.#queue.request(root, { first: true });
  }

  /** The tree of the repository on screen was read again: its nodes have no watcher of their
      own, so theirs and its top's pulses follow (R-542). */
  again(roots: readonly string[]): void {
    for (const root of roots) {
      if (root !== this.#owned) this.#queue.request(root);
    }
  }

  /** Cogit fetched or pulled there: the tracking ref speaks for the server again. Its own
      network commands run quiet, so no watcher event says so. */
  fetched(root: string): void {
    this.#setAhead(root, false);
  }

  /** Its refs moved outside Cogit: a fetch in a terminal, or only a commit. The server is
      asked again rather than taken to have nothing more (R-354). */
  refsMoved(root: string): void {
    if (this.remoteAhead.has(root)) this.#queue.request(root, { fetch: true, first: true });
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
