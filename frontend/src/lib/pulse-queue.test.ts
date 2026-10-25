import { describe, expect, it } from "vitest";
import { BUSY_POLL_MS, GAP_MS, PARALLEL, PulseQueue, type PulseDeps } from "./pulse-queue";
import type { RepoPulse } from "$lib/ipc/bindings";

const PULSE: RepoPulse = {
  missing: false,
  branch: "main",
  branchAt: null,
  tracked: true,
  ahead: 0,
  behind: 1,
  conflicted: false,
  dirty: false,
};

/** Every read waits for the test to answer it, so what runs when is visible. */
function harness(over: Partial<PulseDeps> = {}) {
  const asked: string[] = [];
  const answers = new Map<string, (pulse: RepoPulse) => void>();
  const pulses = new Map<string, RepoPulse>();
  const fetched: [string, boolean][] = [];
  const sleeps: number[] = [];
  let inFlight = 0;
  let widest = 0;
  const state = { busy: false, owned: new Set<string>() };
  const queue = new PulseQueue({
    pulse: (root) => {
      asked.push(root);
      inFlight += 1;
      widest = Math.max(widest, inFlight);
      return new Promise((resolve) =>
        answers.set(root, (pulse) => {
          inFlight -= 1;
          resolve(pulse);
        }),
      );
    },
    fetch: async () => true,
    busy: () => state.busy,
    owned: (root) => state.owned.has(root),
    // A macrotask, as a real timer is: a busy loop on microtasks would never let go.
    sleep: (ms) => {
      sleeps.push(ms);
      return new Promise((resolve) => setTimeout(resolve, 0));
    },
    onPulse: (root, pulse) => void pulses.set(root, pulse),
    onFetch: (root, ok) => void fetched.push([root, ok]),
    ...over,
  });
  const settle = async () => {
    for (let turn = 0; turn < 4; turn += 1) await new Promise((resolve) => setTimeout(resolve, 0));
  };
  const answer = async (root: string, pulse = PULSE) => {
    const reply = answers.get(root);
    if (!reply) throw new Error(`${root} was not asked; asked: ${asked.join(", ")}`);
    answers.delete(root);
    reply(pulse);
    await settle();
  };
  return {
    queue,
    asked,
    pulses,
    fetched,
    sleeps,
    state,
    answer,
    settle,
    widest: () => widest,
  };
}

describe("the status queue of the Repositories list", () => {
  // The rows are what the list shows at once: a few local reads side by side, no pause.
  it("reads several repositories at once, in order, without pausing", async () => {
    const h = harness();
    const roots = ["/a", "/b", "/c", "/d", "/e", "/f"];
    for (const root of roots) h.queue.request(root);
    await h.settle();
    expect(h.asked).toEqual(roots.slice(0, PARALLEL));
    await h.answer("/b");
    expect(h.asked).toEqual(roots.slice(0, PARALLEL + 1));
    for (const root of ["/a", "/c", "/d", "/e", "/f"]) await h.answer(root);
    expect(h.widest()).toBe(PARALLEL);
    expect(h.sleeps).toEqual([]);
    expect([...h.pulses.keys()].sort()).toEqual(roots);
  });

  // Reads are cheap and local; only the server's lane waits for the repository on screen.
  it("reads while the repository on screen is busy, but asks no server then", async () => {
    const asked: string[] = [];
    const h = harness({
      fetch: async (root) => {
        asked.push(root);
        return true;
      },
    });
    h.state.busy = true;
    h.queue.request("/a");
    h.queue.request("/b", { fetch: true });
    await h.settle();
    expect(h.asked).toEqual(["/a"]);
    expect(asked).toEqual([]);
    expect(h.sleeps.every((ms) => ms === BUSY_POLL_MS)).toBe(true);

    h.state.busy = false;
    await h.settle();
    expect(asked).toEqual(["/b"]);
  });

  it("asks the servers one at a time, pausing between two, and holds no read up meanwhile", async () => {
    const replies = new Map<string, (ok: boolean) => void>();
    const h = harness({ fetch: (root) => new Promise((resolve) => replies.set(root, resolve)) });
    h.queue.request("/a", { fetch: true });
    h.queue.request("/b", { fetch: true });
    h.queue.request("/c");
    await h.settle();
    expect([...replies.keys()]).toEqual(["/a"]);
    expect(h.asked).toEqual(["/c"]);
    replies.get("/a")?.(true);
    await h.settle();
    expect([...replies.keys()]).toEqual(["/a", "/b"]);
    expect(h.sleeps.filter((ms) => ms === GAP_MS)).toHaveLength(1);
  });

  it("asks once for a waiting root requested twice", async () => {
    const h = harness();
    const roots = ["/a", "/b", "/c", "/d", "/e"];
    for (const root of roots) h.queue.request(root);
    h.queue.request("/e");
    await h.settle();
    for (const root of roots) await h.answer(root);
    await h.settle();
    expect(h.asked).toEqual(roots);
  });

  it("puts a repository that just changed first", async () => {
    const h = harness();
    const roots = ["/a", "/b", "/c", "/d", "/e", "/f"];
    for (const root of roots) h.queue.request(root);
    h.queue.request("/f", { first: true });
    await h.settle();
    await h.answer("/a");
    expect(h.asked).toEqual([...roots.slice(0, PARALLEL), "/f"]);
  });

  it("drops the answer of a cancelled read and never asks for a cancelled one", async () => {
    const h = harness();
    const roots = ["/a", "/b", "/c", "/d", "/e"];
    for (const root of roots) h.queue.request(root);
    await h.settle();
    h.queue.cancel("/a");
    h.queue.cancel("/e");
    await h.answer("/a");
    await h.settle();
    expect(h.pulses.size).toBe(0);
    expect(h.asked).toEqual(roots.slice(0, PARALLEL));
  });

  it("drops everything on stop, and a later request starts afresh", async () => {
    const h = harness();
    h.queue.request("/a");
    h.queue.request("/b");
    await h.settle();
    h.queue.stop();
    await h.answer("/a");
    expect(h.pulses.size).toBe(0);
    expect(h.queue.pending).toEqual([]);

    h.queue.request("/c");
    await h.settle();
    await h.answer("/c");
    expect([...h.pulses.keys()]).toEqual(["/c"]);
  });

  it("fetches before reading, and a failed fetch is reported, not thrown", async () => {
    const h = harness({ fetch: async (root) => root !== "/b" });
    h.queue.request("/a", { fetch: true });
    h.queue.request("/b", { fetch: true });
    await h.settle();
    await h.answer("/a");
    await h.answer("/b");
    expect(h.fetched).toEqual([
      ["/a", true],
      ["/b", false],
    ]);
    expect(h.pulses.has("/b")).toBe(true);
  });

  it("fetches the repository on screen but leaves its status to the panels", async () => {
    const h = harness();
    h.state.owned.add("/a");
    h.queue.request("/a", { fetch: true });
    h.queue.request("/b");
    await h.settle();
    expect(h.fetched).toEqual([["/a", true]]);
    expect(h.asked).toEqual(["/b"]);
  });
});
