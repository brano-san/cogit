/** Renderer-side memory metrics for `kind=mem` in the profile log (doc/14-profiling.md).
    A heap size on its own does not say what grew, so every sample carries the counters
    that distinguish the three diagnoses: time, actions, or one operation. */

/** Chromium-only and absent unless the renderer exposes it; WebView2 does. */
export interface HeapReading {
  usedJSHeapSize: number;
  totalJSHeapSize: number;
  jsHeapSizeLimit: number;
}

export interface ProbeSources {
  heap: () => HeapReading | undefined;
  domNodes: () => number;
  listeners: () => number;
  caches: () => Record<string, number>;
}

export interface MemorySample {
  usedHeapKib: number;
  totalHeapKib: number;
  limitKib: number;
  domNodes: number;
  listeners: number;
  caches: Record<string, number>;
}

export interface Scheduler {
  schedule: (tick: () => void, ms: number) => ReturnType<typeof setInterval>;
  cancel: (handle: ReturnType<typeof setInterval>) => void;
}

/** Ten seconds: slow enough to cost nothing, dense enough that an hour of work still
    draws a curve rather than four points. */
export const PROBE_INTERVAL_MS = 10_000;

const KIB = 1024;

/** A probe that throws takes down the thing it was watching, so every read is guarded
    and a lost figure reads as zero rather than as a missing sample. */
function read<T>(source: () => T, fallback: T): T {
  try {
    return source();
  } catch {
    return fallback;
  }
}

function count(value: number): number {
  return Number.isFinite(value) && value > 0 ? Math.round(value) : 0;
}

function kib(bytes: number): number {
  return count(bytes / KIB);
}

export function sample(sources: ProbeSources): MemorySample {
  const heap = read(sources.heap, undefined);

  return {
    usedHeapKib: kib(heap?.usedJSHeapSize ?? 0),
    totalHeapKib: kib(heap?.totalJSHeapSize ?? 0),
    limitKib: kib(heap?.jsHeapSizeLimit ?? 0),
    domNodes: count(read(sources.domNodes, 0)),
    listeners: count(read(sources.listeners, 0)),
    caches: read(sources.caches, {}),
  };
}

/** Wrapped rather than passed by reference: a bare `setInterval` is detached from
    `window`, and Chromium answers that with `TypeError: Illegal invocation`. */
const DEFAULT_SCHEDULER: Scheduler = {
  schedule: (tick, ms) => setInterval(tick, ms),
  cancel: (handle) => clearInterval(handle),
};

/** Returns the function that stops sampling. */
export function start(
  sources: ProbeSources,
  sink: (taken: MemorySample) => void,
  scheduler: Scheduler = DEFAULT_SCHEDULER,
): () => void {
  const handle = scheduler.schedule(() => {
    const taken = sample(sources);
    try {
      sink(taken);
    } catch {
      // A sample that cannot be delivered is not worth stopping the probe over.
    }
  }, PROBE_INTERVAL_MS);

  return () => scheduler.cancel(handle);
}

/** The live sources, for the app. Kept apart from `sample` so the arithmetic above
    stays testable without a document or a backend. */
export function browserSources(
  listeners: () => number,
  caches: () => Record<string, number>,
): ProbeSources {
  return {
    heap: () => (performance as Performance & { memory?: HeapReading }).memory,
    domNodes: () => document.getElementsByTagName("*").length,
    listeners,
    caches,
  };
}
