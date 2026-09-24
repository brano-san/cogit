import { describe, expect, it } from "vitest";
import { BUSY_POLL_MS, GAP_MS, PulseQueue, type PulseDeps } from "./pulse-queue";
import type { RepoPulse } from "$lib/ipc/bindings";

const PULSE: RepoPulse = {
  missing: false,
  branch: "main",
  tracked: true,
  ahead: 0,
  behind: 1,
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
  it("reads one repository at a time, pausing between two", async () => {
    const h = harness();
    h.queue.request("/a");
    h.queue.request("/b");
    h.queue.request("/c");
    await h.settle();
    expect(h.asked).toEqual(["/a"]);
    await h.answer("/a");
    await h.answer("/b");
    await h.answer("/c");
    expect(h.asked).toEqual(["/a", "/b", "/c"]);
    expect(h.widest()).toBe(1);
    expect(h.sleeps.filter((ms) => ms === GAP_MS)).toHaveLength(3);
    expect([...h.pulses.keys()]).toEqual(["/a", "/b", "/c"]);
  });

  // The requirement: background rows never slow the repository on screen down.
  it("starts nothing while the repository on screen is busy", async () => {
    const h = harness();
    h.state.busy = true;
    h.queue.request("/a");
    await h.settle();
    await h.settle();
    expect(h.asked).toEqual([]);
    expect(h.sleeps.every((ms) => ms === BUSY_POLL_MS)).toBe(true);

    h.state.busy = false;
    await h.settle();
    expect(h.asked).toEqual(["/a"]);
  });

  it("finishes the read under way but starts no next one once the screen is busy", async () => {
    const h = harness();
    h.queue.request("/a");
    h.queue.request("/b");
    await h.settle();
    h.state.busy = true;
    await h.answer("/a");
    await h.settle();
    expect(h.asked).toEqual(["/a"]);
    h.state.busy = false;
    await h.settle();
    expect(h.asked).toEqual(["/a", "/b"]);
  });

  it("asks once for a root requested twice", async () => {
    const h = harness();
    h.queue.request("/a");
    h.queue.request("/b");
    h.queue.request("/b");
    await h.answer("/a");
    await h.answer("/b");
    await h.settle();
    expect(h.asked).toEqual(["/a", "/b"]);
  });

  it("puts a repository that just changed first", async () => {
    const h = harness();
    h.queue.request("/a");
    h.queue.request("/b");
    h.queue.request("/c");
    h.queue.request("/c", { first: true });
    await h.answer("/a");
    expect(h.asked).toEqual(["/a", "/c"]);
  });

  it("drops the answer of a cancelled read and never asks for a cancelled one", async () => {
    const h = harness();
    h.queue.request("/a");
    h.queue.request("/b");
    await h.settle();
    h.queue.cancel("/a");
    h.queue.cancel("/b");
    await h.answer("/a");
    await h.settle();
    expect(h.pulses.size).toBe(0);
    expect(h.asked).toEqual(["/a"]);
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
