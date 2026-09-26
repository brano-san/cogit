import { describe, expect, it } from "vitest";
import { activity, applyOperation, busyLabel, cancellable, trackCancellable } from "./operations";

const start = (id: number, label: string) => ({ id, label, success: null });
const end = (id: number, success: boolean) => ({ id, label: "", success });

describe("applyOperation", () => {
  it("records an operation that started", () => {
    expect(applyOperation(new Map(), start(1, "Fetching")).get(1)).toBe("Fetching");
  });

  it("forgets one that finished", () => {
    const running = applyOperation(new Map(), start(1, "Fetching"));
    expect(applyOperation(running, end(1, true)).size).toBe(0);
  });

  it("forgets one that failed just the same", () => {
    const running = applyOperation(new Map(), start(1, "Pushing"));
    expect(applyOperation(running, end(1, false)).size).toBe(0);
  });

  it("keeps the others when one finishes", () => {
    let running = applyOperation(new Map(), start(1, "Fetching"));
    running = applyOperation(running, start(2, "Pushing"));
    expect(applyOperation(running, end(1, true)).get(2)).toBe("Pushing");
  });

  it("ignores an end for something it never saw start", () => {
    expect(applyOperation(new Map(), end(9, true)).size).toBe(0);
  });

  it("does not mutate the map it was given", () => {
    const running = new Map([[1, "Fetching"]]);
    applyOperation(running, end(1, true));
    expect(running.size).toBe(1);
  });
});

describe("busyLabel", () => {
  it("is empty when nothing is running", () => {
    expect(busyLabel(new Map())).toBeNull();
  });

  it("names the single running operation", () => {
    expect(busyLabel(new Map([[1, "Fetching"]]))).toBe("Fetching…");
  });

  it("counts when several run at once", () => {
    const running = new Map([
      [1, "Fetching"],
      [2, "Pushing"],
    ]);
    expect(busyLabel(running)).toBe("2 operations running…");
  });
});

describe("activity", () => {
  const idle = { operations: new Map(), opening: false, failed: false };

  it("is ready when nothing happens", () => {
    expect(activity(idle)).toEqual({ label: "Ready", busy: false, tone: "idle" });
  });

  it("reports the error state when the last action failed", () => {
    expect(activity({ ...idle, failed: true })).toEqual({
      label: "Error",
      busy: false,
      tone: "error",
    });
  });

  it("reports opening a repository", () => {
    expect(activity({ ...idle, opening: true }).label).toBe("Opening repository…");
  });

  it("reports a tracked backend operation", () => {
    const operations = new Map([[1, "Rebasing"]]);
    expect(activity({ ...idle, operations })).toEqual({
      label: "Rebasing…",
      busy: true,
      tone: "busy",
    });
  });

  it("prefers network progress over everything else", () => {
    const operations = new Map([[1, "Rebasing"]]);
    const state = { ...idle, operations, network: "pull", networkProgress: "Receiving 40%" };
    expect(activity(state).label).toBe("Receiving 40%");
  });

  it("names the network operation until it reports progress", () => {
    expect(activity({ ...idle, network: "push" }).label).toBe("push…");
  });

  it("stays busy while an operation runs even if something failed earlier", () => {
    const operations = new Map([[1, "Fetching"]]);
    expect(activity({ ...idle, operations, failed: true }).tone).toBe("busy");
  });
});

describe("activity · bulk work", () => {
  const idle = { operations: new Map(), opening: false, failed: false };

  it("shows how far a bulk run has got", () => {
    expect(activity({ ...idle, bulk: { label: "Fetching", done: 2, total: 7 } }).label).toBe(
      "Fetching 2 of 7…",
    );
  });

  it("counts the ones that failed without stopping", () => {
    const bulk = { label: "Fetching", done: 7, total: 7, failed: 2 };
    expect(activity({ ...idle, bulk }).label).toBe("Fetching 7 of 7… (2 failed)");
  });

  it("outranks a single operation, because it is the thing the user started", () => {
    const operations = new Map([[1, "Fetching"]]);
    const bulk = { label: "Fetching", done: 1, total: 4 };
    expect(activity({ ...idle, operations, bulk }).label).toBe("Fetching 1 of 4…");
  });

  it("is busy while it runs", () => {
    expect(activity({ ...idle, bulk: { label: "Fetching", done: 0, total: 3 } }).busy).toBe(true);
  });
});

describe("trackCancellable", () => {
  const step = (id: number, kind: string, phase: "queued" | "running" | "done") => ({
    id,
    kind: kind as "fetch",
    phase,
  });

  it("holds a fetch, pull or push once it runs, not while it waits", () => {
    let ids = trackCancellable([], step(1, "fetch", "queued"));
    expect(ids).toEqual([]);
    ids = trackCancellable(ids, step(1, "fetch", "running"));
    ids = trackCancellable(ids, step(2, "pull", "running"));
    ids = trackCancellable(ids, step(3, "push", "running"));
    expect(ids).toEqual([1, 2, 3]);
  });

  it("leaves out what does not talk to a server", () => {
    expect(trackCancellable([], step(4, "commit", "running"))).toEqual([]);
  });

  it("lets one go when it is done, whatever it was", () => {
    const ids = trackCancellable([1, 2], step(1, "fetch", "done"));
    expect(ids).toEqual([2]);
  });

  it("names the one that started last as the one Cancel stops", () => {
    expect(cancellable([1, 2])).toBe(2);
    expect(cancellable([])).toBeNull();
  });
});
