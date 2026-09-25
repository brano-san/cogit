import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ChangeKind } from "./ipc";
import { DiskPasses } from "./disk-refresh";
import { clear, mark } from "./staleness";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

function passes() {
  const runs: ChangeKind[][] = [];
  let finish: () => void = () => {};
  let seen: () => ReadonlySet<ChangeKind> = () => new Set();
  let running = 0;
  let overlapped = false;
  const disk = new DiskPasses(120, (kinds, arrived) => {
    runs.push([...kinds].sort());
    seen = arrived;
    running += 1;
    if (running > 1) overlapped = true;
    return new Promise<void>((resolve) => {
      finish = () => {
        running -= 1;
        resolve();
      };
    });
  });
  return { disk, runs, finish: () => finish(), seen: () => seen(), overlapped: () => overlapped };
}

// Writes into .vs/ or .svelte-kit/ never stop: each quiet spell after one started another
// full refresh while the last was still reading, and the old one's freshen put out the
// stale dots events had lit meanwhile.
describe("refreshing after changes on disk", () => {
  it("answers a burst once, after it settles", async () => {
    const { disk, runs } = passes();

    disk.add("index");
    disk.add("workingTree");
    await vi.advanceTimersByTimeAsync(100);
    disk.add("head");
    await vi.advanceTimersByTimeAsync(119);
    expect(runs).toEqual([]);

    await vi.advanceTimersByTimeAsync(1);
    expect(runs).toEqual([["head", "index", "workingTree"]]);
  });

  it("never runs two passes at once, and runs one more for what came during the first", async () => {
    const t = passes();
    t.disk.add("workingTree");
    await vi.advanceTimersByTimeAsync(120);

    t.disk.add("index");
    await vi.advanceTimersByTimeAsync(500);
    expect(t.runs).toHaveLength(1);

    t.finish();
    await vi.advanceTimersByTimeAsync(0);
    expect(t.runs).toEqual([["workingTree"], ["index"]]);
    expect(t.overlapped()).toBe(false);
  });

  it("tells the pass what arrived while it ran", async () => {
    const t = passes();
    t.disk.add("workingTree");
    await vi.advanceTimersByTimeAsync(120);

    t.disk.add("index");

    expect([...t.seen()]).toEqual(["index"]);
  });

  it("does not run again when nothing came during the pass", async () => {
    const t = passes();
    t.disk.add("refs");
    await vi.advanceTimersByTimeAsync(120);
    t.finish();
    await vi.advanceTimersByTimeAsync(1_000);

    expect(t.runs).toHaveLength(1);
  });
});

describe("clearing the stale dots at the end of a step", () => {
  it("keeps the ones that events of the meantime lit again", () => {
    let stale = mark(new Set(), "workingTree");

    stale = clear(stale, ["files", "diff", "repositories"], ["index"]);

    expect([...stale].sort()).toEqual(["diff", "files"]);
  });
});
