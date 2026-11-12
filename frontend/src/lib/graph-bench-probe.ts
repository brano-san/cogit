// Test-only helpers for the graph's allocation tests (doc/15-benchmark.md, §10): what a frame
// of the graph costs in garbage, measured in Node, where a test can see it.
import { Session } from "node:inspector/promises";

interface ProfileNode {
  selfSize: number;
  children: ProfileNode[];
}

const total = (node: ProfileNode): number => node.children.reduce((sum, child) => sum + total(child), node.selfSize);

/** Bytes `run` allocated, collected or not: V8's sampling heap profiler, whose samples are
    scaled to an estimate of every allocation. `interval` is the mean bytes between samples. */
export async function allocatedBytes(run: () => void | Promise<void>, interval = 512): Promise<number> {
  const session = new Session();
  session.connect();
  try {
    await session.post("HeapProfiler.enable");
    await session.post("HeapProfiler.startSampling", {
      samplingInterval: interval,
      includeObjectsCollectedByMajorGC: true,
      includeObjectsCollectedByMinorGC: true,
    });
    await run();
    const { profile } = (await session.post("HeapProfiler.stopSampling")) as unknown as { profile: { head: ProfileNode } };
    return total(profile.head);
  } finally {
    await session.post("HeapProfiler.disable").catch(() => {});
    session.disconnect();
  }
}

/** Mean milliseconds per call of `run` over `times` calls, after `warmup` unrecorded ones. */
export function msPerCall(run: () => void, times = 500, warmup = 100): number {
  for (let i = 0; i < warmup; i++) run();
  const start = performance.now();
  for (let i = 0; i < times; i++) run();
  return (performance.now() - start) / times;
}

/** Numbers printed only when asked for: `GRAPH_BENCH=1 npx vitest run <file>`. */
export const BENCH = typeof process !== "undefined" && process.env.GRAPH_BENCH === "1";

export function report(label: string, values: Record<string, number>): void {
  if (!BENCH) return;
  const parts = Object.entries(values).map(([key, value]) => `${key}=${Math.round(value * 1000) / 1000}`);
  console.log(`[graph-bench] ${label}: ${parts.join(" ")}`);
}
