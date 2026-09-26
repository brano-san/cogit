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

  // A commit made in a terminal while Cogit was in the background: the focus came back
  // inside the minute, and the row waited for the next focus change after it.
  it("are read once the minute is over when the focus came back inside it", async () => {
    vi.useFakeTimers();
    vi.resetModules();
    const { repoPulse } = await import("./repo-pulse.svelte");
    repoPulse.setOwned("C:/repos/shown");
    repoPulse.watch(["C:/repos/shown", "C:/repos/left"]);
    await vi.advanceTimersByTimeAsync(2_000);
    repoPulse.revisit();
    await vi.advanceTimersByTimeAsync(2_000);
    readPulse.mockClear();

    repoPulse.revisit();
    repoPulse.revisit();
    await vi.advanceTimersByTimeAsync(2_000);
    expect(readPulse).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(60_000);
    expect(readPulse.mock.calls).toEqual([["C:/repos/left"]]);
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

// A server that takes the connection and never answers held each probe past its 120 s, and
// every tick started one more: an hour of it was dozens of `git ls-remote` processes.
describe("a server that does not answer", () => {
  beforeAll(() => import("./repo-pulse.svelte"), 60_000);
  afterEach(() => vi.useRealTimers());

  async function silentServer() {
    vi.useFakeTimers();
    vi.resetModules();
    const ipc = await import("$lib/ipc/repo-rows");
    const probe = vi.mocked(ipc.pullProbe);
    const answers: ((ahead: boolean) => void)[] = [];
    probe.mockClear();
    probe.mockImplementation(() => new Promise<boolean>((resolve) => answers.push(resolve)) as never);
    const { repoPulse } = await import("./repo-pulse.svelte");
    repoPulse.watch(["C:/repos/silent"]);
    repoPulse.fetchEvery(1);
    const done = () => {
      repoPulse.fetchEvery(0);
      probe.mockImplementation(async () => false);
    };
    return { repoPulse, probe, answers, done };
  }

  it("is asked again only once the probe before has ended", async () => {
    const { probe, answers, done } = await silentServer();

    await vi.advanceTimersByTimeAsync(5 * 61_000);
    expect(probe).toHaveBeenCalledTimes(1);

    answers[0]?.(false);
    await vi.advanceTimersByTimeAsync(61_000);
    expect(probe).toHaveBeenCalledTimes(2);
    done();
  });

  it("is not listened to once the wait for it was given up", async () => {
    const { repoPulse, answers, done } = await silentServer();
    await vi.advanceTimersByTimeAsync(61_000 + 121_000);
    expect(repoPulse.unknown.has("C:/repos/silent")).toBe(true);

    answers[0]?.(true);
    await vi.advanceTimersByTimeAsync(1_000);

    expect(repoPulse.remoteAhead.has("C:/repos/silent")).toBe(false);
    done();
  });
});

// The pull arrow a background check put up stayed after Cogit's own Pull, which runs quiet
// and moves no watcher; a commit made in a terminal took it down though nothing was fetched.
describe("the server-ahead arrow", () => {
  afterEach(() => vi.useRealTimers());

  it("goes once Cogit itself fetched or pulled there", async () => {
    vi.resetModules();
    const { repoPulse } = await import("./repo-pulse.svelte");
    repoPulse.remoteAhead = new Set(["C:/repos/one"]);

    repoPulse.fetched("C:/repos/one");

    expect(repoPulse.remoteAhead.has("C:/repos/one")).toBe(false);
  });

  it("is checked with the server again when the refs move outside Cogit, not dropped", async () => {
    vi.useFakeTimers();
    vi.resetModules();
    const ipc = await import("$lib/ipc/repo-rows");
    const { repoPulse } = await import("./repo-pulse.svelte");
    repoPulse.watch(["C:/repos/one"]);
    repoPulse.fetchEvery(5);
    await vi.advanceTimersByTimeAsync(2_000);
    vi.mocked(ipc.pullProbe).mockClear();
    vi.mocked(ipc.pullProbe).mockResolvedValueOnce(true as never);
    repoPulse.remoteAhead = new Set(["C:/repos/one"]);

    repoPulse.refsMoved("C:/repos/one");
    expect(repoPulse.remoteAhead.has("C:/repos/one")).toBe(true);
    await vi.advanceTimersByTimeAsync(2_000);

    expect(ipc.pullProbe).toHaveBeenCalledWith("C:/repos/one");
    expect(repoPulse.remoteAhead.has("C:/repos/one")).toBe(true);
    repoPulse.fetchEvery(0);
  });
});

// A submodule of the repository on screen has no watcher of its own: its marks follow the
// re-reads of the tree its parent's watcher and writes set off (R-542).
describe("the nodes of the tree the panels own", () => {
  beforeAll(() => import("./repo-pulse.svelte"), 60_000);
  afterEach(() => vi.useRealTimers());

  it("are read again when the tree is, all but the one the panels show", async () => {
    vi.useFakeTimers();
    vi.resetModules();
    const { repoPulse } = await import("./repo-pulse.svelte");
    const nodes = ["C:/repos/app/vendor/lib", "C:/repos/app/docs"];
    repoPulse.setOwned("C:/repos/app/docs");
    repoPulse.watch(["C:/repos/app", ...nodes]);
    await vi.advanceTimersByTimeAsync(2_000);
    readPulse.mockClear();

    repoPulse.again(["C:/repos/app", ...nodes]);
    await vi.advanceTimersByTimeAsync(2_000);

    expect(readPulse.mock.calls).toEqual([["C:/repos/app"], ["C:/repos/app/vendor/lib"]]);
  });

  // A status of every node right after an open or a click held the next click up: 12 ms to
  // select a commit became 145 on a repository with submodules (A/B of 26.09, R-617).
  it("wait for the reads of the tree to stop a while, not the action that read it", async () => {
    vi.useFakeTimers();
    vi.resetModules();
    const { repoPulse } = await import("./repo-pulse.svelte");
    repoPulse.setOwned("C:/repos/app");
    repoPulse.watch(["C:/repos/app"], ["C:/repos/app/lib"]);
    await vi.advanceTimersByTimeAsync(100);
    repoPulse.again(["C:/repos/app/lib"]);

    await vi.advanceTimersByTimeAsync(1_450);
    expect(readPulse).not.toHaveBeenCalledWith("C:/repos/app/lib");

    await vi.advanceTimersByTimeAsync(1_000);
    expect(readPulse).toHaveBeenCalledWith("C:/repos/app/lib");
  });
});

// Leaving a submodule, its node had no marks at all until its pulse was read: after the
// 500 ms and after the open of the repository clicked, which holds the queue (R-542).
describe("the row the panels let go of", () => {
  beforeAll(() => import("./repo-pulse.svelte"), 60_000);
  afterEach(() => vi.useRealTimers());

  it("keeps the marks they showed until its own pulse comes", async () => {
    vi.useFakeTimers();
    vi.resetModules();
    const { repoPulse } = await import("./repo-pulse.svelte");
    const shown = { missing: false, branch: "dev", tracked: true, ahead: 1, behind: 0, dirty: true };
    repoPulse.watch(["C:/repos/app/vendor/lib", "C:/repos/other"]);
    repoPulse.setOwned("C:/repos/app/vendor/lib", shown);

    repoPulse.setOwned("C:/repos/other", null);

    expect(repoPulse.pulses.get("C:/repos/app/vendor/lib")).toEqual(shown);
  });

  it("takes the marks of the latest read of the panels, not of the first", async () => {
    vi.useFakeTimers();
    vi.resetModules();
    const { repoPulse } = await import("./repo-pulse.svelte");
    const clean = { missing: false, branch: "dev", tracked: false, ahead: 0, behind: 0, dirty: false };
    repoPulse.watch(["C:/repos/one", "C:/repos/two"]);
    repoPulse.setOwned("C:/repos/one", clean);
    repoPulse.setOwned("C:/repos/one", { ...clean, dirty: true });

    repoPulse.setOwned("C:/repos/two", null);

    expect(repoPulse.pulses.get("C:/repos/one")?.dirty).toBe(true);
  });
});

describe("a row removed from the list", () => {
  beforeAll(() => import("./repo-pulse.svelte"), 60_000);

  // Added back, its nodes showed what was read before the removal, and were not read again.
  it("takes what was read of its submodule nodes with it", async () => {
    vi.resetModules();
    const { repoPulse } = await import("./repo-pulse.svelte");
    const node = { missing: false, branch: null, tracked: false, ahead: 0, behind: 0, dirty: true };
    repoPulse.pulses = new Map([
      ["C:/repos/app/vendor/lib", node],
      ["C:/repos/application", node],
    ]);
    repoPulse.unknown = new Set(["C:/repos/app/vendor/lib"]);

    repoPulse.forget("C:/repos/app");

    expect([...repoPulse.pulses.keys()]).toEqual(["C:/repos/application"]);
    expect(repoPulse.unknown.size).toBe(0);
  });
});
