import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

const readPulse = vi.fn(async () => ({ missing: false }));
vi.mock("$lib/ipc/repo-rows", () => ({ repoPulse: readPulse, pullProbe: vi.fn(async () => false) }));

describe("the row a repository leaves on screen", () => {
  beforeAll(() => import("./repo-pulse.svelte"), 60_000);
  afterEach(() => vi.useRealTimers());

  // Reading it inside the close was the `repo.close` slowdown of 24.09.
  it("is read once the close has settled, not during it", async () => {
    vi.useFakeTimers();
    vi.resetModules();
    const { repoPulse } = await import("./repo-pulse.svelte");
    repoPulse.setOwned("C:/repos/one");
    repoPulse.watch(["C:/repos/one"]);
    await vi.advanceTimersByTimeAsync(2_000);
    readPulse.mockClear();

    repoPulse.setOwned(null);
    await vi.advanceTimersByTimeAsync(100);
    expect(readPulse).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1_000);
    expect(readPulse).toHaveBeenCalledWith("C:/repos/one");
  });
});

// `?` and the pull arrow stayed on a row until a restart once it was removed, or the
// background check was turned off.
describe("what the background check learnt of a row", () => {
  beforeAll(() => import("./repo-pulse.svelte"), 60_000);

  it("goes with the row", async () => {
    vi.resetModules();
    const { repoPulse } = await import("./repo-pulse.svelte");
    repoPulse.unknown = new Set(["C:/repos/gone"]);
    repoPulse.remoteAhead = new Set(["C:/repos/gone"]);

    repoPulse.forget("C:/repos/gone");

    expect(repoPulse.unknown.has("C:/repos/gone")).toBe(false);
    expect(repoPulse.remoteAhead.has("C:/repos/gone")).toBe(false);
  });

  it("goes when the check is turned off", async () => {
    vi.resetModules();
    const { repoPulse } = await import("./repo-pulse.svelte");
    repoPulse.fetchEvery(5);
    repoPulse.unknown = new Set(["C:/repos/one"]);
    repoPulse.remoteAhead = new Set(["C:/repos/two"]);

    repoPulse.fetchEvery(0);

    expect(repoPulse.unknown.size).toBe(0);
    expect(repoPulse.remoteAhead.size).toBe(0);
  });

  it("is not written by a probe that answers after the row was removed", async () => {
    vi.useFakeTimers();
    vi.resetModules();
    const ipc = await import("$lib/ipc/repo-rows");
    let answer: (ahead: boolean) => void = () => {};
    vi.mocked(ipc.pullProbe).mockImplementationOnce(
      () => new Promise<boolean>((resolve) => (answer = resolve)) as never,
    );
    const { repoPulse } = await import("./repo-pulse.svelte");
    repoPulse.watch(["C:/repos/slow"]);
    repoPulse.fetchEvery(1);
    await vi.advanceTimersByTimeAsync(61_000);

    repoPulse.forget("C:/repos/slow");
    answer(true);
    await vi.advanceTimersByTimeAsync(1_000);

    expect(repoPulse.remoteAhead.has("C:/repos/slow")).toBe(false);
    repoPulse.fetchEvery(0);
  });
});

// A row the panels take over keeps the pulse read before: released again, it showed that
// old dot and arrow for seconds, until the next read.
describe("a row the panels take over", () => {
  beforeAll(() => import("./repo-pulse.svelte"), 60_000);

  it("drops the pulse read while it was in the list only", async () => {
    vi.resetModules();
    const { repoPulse } = await import("./repo-pulse.svelte");
    repoPulse.pulses = new Map([["C:/repos/a", { missing: false } as never]]);

    repoPulse.setOwned("C:/repos/a");

    expect(repoPulse.pulses.has("C:/repos/a")).toBe(false);
  });
});

// A closed row has no watcher: it was read once a session, so a file changed in it
// outside Cogit never lit its dot until a restart (F-451).
describe("the rows no watcher covers", () => {
  beforeAll(() => import("./repo-pulse.svelte"), 60_000);
  afterEach(() => vi.useRealTimers());

  it("are read again when the window comes back into focus", async () => {
    vi.useFakeTimers();
    vi.resetModules();
    const { repoPulse } = await import("./repo-pulse.svelte");
    repoPulse.watch(["C:/repos/closed"]);
    await vi.advanceTimersByTimeAsync(2_000);
    readPulse.mockClear();

    repoPulse.revisit();
    await vi.advanceTimersByTimeAsync(2_000);

    expect(readPulse).toHaveBeenCalledWith("C:/repos/closed");
  });

  it("are not read again on every focus change", async () => {
    vi.useFakeTimers();
    vi.resetModules();
    const { repoPulse } = await import("./repo-pulse.svelte");
    repoPulse.watch(["C:/repos/closed"]);
    await vi.advanceTimersByTimeAsync(2_000);
    repoPulse.revisit();
    await vi.advanceTimersByTimeAsync(2_000);
    readPulse.mockClear();

    repoPulse.revisit();
    await vi.advanceTimersByTimeAsync(2_000);

    expect(readPulse).not.toHaveBeenCalled();
  });

  it("leave out the one the panels own", async () => {
    vi.useFakeTimers();
    vi.resetModules();
    const { repoPulse } = await import("./repo-pulse.svelte");
    repoPulse.setOwned("C:/repos/open");
    repoPulse.watch(["C:/repos/open"]);
    await vi.advanceTimersByTimeAsync(2_000);
    readPulse.mockClear();

    repoPulse.revisit();
    await vi.advanceTimersByTimeAsync(2_000);

    expect(readPulse).not.toHaveBeenCalled();
  });
});
