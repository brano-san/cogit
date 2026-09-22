import { describe, expect, it } from "vitest";
import {
  PROBE_INTERVAL_MS,
  sample,
  start,
  type MemorySample,
  type ProbeSources,
  type Scheduler,
} from "$lib/mem-probe";

function sources(over: Partial<ProbeSources> = {}): ProbeSources {
  return {
    heap: () => ({
      usedJSHeapSize: 2 * 1024 * 1024,
      totalJSHeapSize: 4 * 1024 * 1024,
      jsHeapSizeLimit: 8 * 1024 * 1024,
    }),
    domNodes: () => 1200,
    listeners: () => 6,
    caches: () => ({ graphRows: 50000, graphSegments: 49999 }),
    ...over,
  };
}

const HANDLE = 7 as unknown as ReturnType<typeof setInterval>;

/** Captures what `start` scheduled, so the test can run a tick without a real clock. */
function fakeScheduler() {
  const state: {
    tick: (() => void) | null;
    everyMs: number;
    cancelled: ReturnType<typeof setInterval> | null;
  } = { tick: null, everyMs: 0, cancelled: null };

  const scheduler: Scheduler = {
    schedule: (tick, ms) => {
      state.tick = tick;
      state.everyMs = ms;
      return HANDLE;
    },
    cancel: (handle) => {
      state.cancelled = handle;
    },
  };

  return { scheduler, state };
}

describe("sample", () => {
  it("reports the heap in KiB", () => {
    const taken = sample(sources());
    expect(taken.usedHeapKib).toBe(2048);
    expect(taken.totalHeapKib).toBe(4096);
    expect(taken.limitKib).toBe(8192);
  });

  it("carries the counters that a heap size alone cannot explain", () => {
    const taken = sample(sources());
    expect(taken.domNodes).toBe(1200);
    expect(taken.listeners).toBe(6);
    expect(taken.caches).toEqual({ graphRows: 50000, graphSegments: 49999 });
  });

  it("still reports the counters where the heap is not exposed", () => {
    const taken = sample(sources({ heap: () => undefined }));
    expect(taken.usedHeapKib).toBe(0);
    expect(taken.limitKib).toBe(0);
    expect(taken.domNodes).toBe(1200);
  });

  it("survives a source that throws rather than losing the whole sample", () => {
    const taken = sample(
      sources({
        domNodes: () => {
          throw new Error("detached document");
        },
      }),
    );
    expect(taken.domNodes).toBe(0);
    expect(taken.usedHeapKib).toBe(2048);
  });

  it("clamps values that cannot be counts", () => {
    const taken = sample(sources({ domNodes: () => Number.NaN, listeners: () => -1 }));
    expect(taken.domNodes).toBe(0);
    expect(taken.listeners).toBe(0);
  });
});

describe("start", () => {
  it("samples on the interval and stops when told", () => {
    const taken: MemorySample[] = [];
    const { scheduler, state } = fakeScheduler();

    const stop = start(sources(), (one) => taken.push(one), scheduler);
    expect(state.everyMs).toBe(PROBE_INTERVAL_MS);

    state.tick?.();
    expect(taken).toHaveLength(1);
    expect(taken[0]?.usedHeapKib).toBe(2048);

    stop();
    expect(state.cancelled).toBe(HANDLE);
  });

  it("keeps sampling after a sink that throws", () => {
    let calls = 0;
    const { scheduler, state } = fakeScheduler();

    start(
      sources(),
      () => {
        calls += 1;
        throw new Error("backend gone");
      },
      scheduler,
    );

    expect(() => state.tick?.()).not.toThrow();
    expect(() => state.tick?.()).not.toThrow();
    expect(calls).toBe(2);
  });
});
