import { describe, expect, it, vi } from "vitest";
import type {
  BlameTables,
  FileRevision,
  OriginCandidate,
  OriginQuery,
  OriginReport,
} from "$lib/ipc/investigate";
import { InvestigateSession, type InvestigateBackend } from "./session.svelte";

function rev(oid: string, path: string): FileRevision {
  return {
    oid,
    parents: [],
    summary: oid,
    author: "A",
    email: "a@x",
    timestamp: 0,
    path,
    previousPath: null,
    change: "modified",
  };
}

/** Three lines, each written by `oid` as one block. */
function tables(oid: string, path: string): BlameTables {
  return {
    commits: [
      { oid, summary: oid, author: "A", email: "a@x", timestamp: 0, merge: false, boundary: false, uncommitted: false },
    ],
    sources: [{ commit: 0, path, previous: { oid: `${oid}^`, path } }],
    lines: [1, 2, 3].map((n) => ({ line: n, text: `t${n}`, source: 0, origLine: n, change: "added" as const })),
  };
}

function candidate(kind: OriginCandidate["kind"], deeper: OriginCandidate["deeper"]): OriginCandidate {
  return { kind, rev: "p", path: "donor.rs", from: 1, to: 3, score: 90, likelihood: "high", deeper, block: [], source: [] };
}

function fakeBackend(report: OriginReport | null = null) {
  const logs: Record<string, FileRevision[]> = {
    "main.rs@": [rev("c2", "main.rs"), rev("c1", "main.rs")],
    "donor.rs@c0^": [rev("c0", "donor.rs")],
    "main.rs@c1^^": [rev("c1", "main.rs")],
  };
  const backend = {
    log: vi.fn(async (path: string, at: string | null) => logs[`${path}@${at ?? ""}`] ?? []),
    blame: vi.fn(async (path: string, at: string | null) => tables(at ?? "wt", path)),
    origins: vi.fn(async (_query: OriginQuery, onStarted: (id: number) => void) => {
      onStarted(7);
      return report;
    }),
    cancel: vi.fn(async () => {}),
  } satisfies InvestigateBackend;
  return backend;
}

const MOVED: OriginReport = {
  candidates: [
    candidate("appeared", null),
    candidate("moved", { rev: "c0^", path: "donor.rs", line: 2 }),
  ],
  best: 1,
};

async function started(backend: InvestigateBackend, line: number | null = null) {
  const session = new InvestigateSession(backend, { path: "main.rs", rev: null, line });
  await session.start();
  return session;
}

describe("Investigate session", () => {
  it("opens on the file's log under a Working Tree row and blames the working tree", async () => {
    const backend = fakeBackend();
    const session = await started(backend);

    expect(session.items.map((item) => item.kind)).toEqual(["header", "workingTree", "commit", "commit"]);
    expect(session.selectedItem).toBe(1);
    expect(backend.blame).toHaveBeenCalledWith("main.rs", null, false);
    expect(session.blame?.lines).toHaveLength(3);
  });

  it("starting from a commit that did not touch the file lands on the version it holds", async () => {
    const backend = fakeBackend();
    const session = new InvestigateSession(backend, { path: "main.rs", rev: "c1^^", line: 3 });
    await session.start();

    expect(session.location).toEqual({ path: "main.rs", rev: "c1", line: 3 });
    expect(session.selectedItem).toBe(1);
    expect(backend.blame).toHaveBeenCalledWith("main.rs", "c1", false);
    expect(backend.origins).toHaveBeenCalledTimes(1);
  });

  it("searches the picked line's block, opens the card and picks the best candidate", async () => {
    const backend = fakeBackend(MOVED);
    const session = await started(backend);

    session.selectLine(1);
    await vi.waitFor(() => expect(session.search).toBe("done"));

    expect(backend.origins.mock.calls[0]![0]).toMatchObject({ commit: "wt", from: 1, to: 3, line: 2 });
    expect(session.cardOpen).toBe(true);
    expect(session.chosen).toBe(1);
    expect(session.perspective, "the origin is elsewhere").toBe("blameOrigins");
    expect(session.location.line).toBe(2);
  });

  it("goes deeper into the source's version, adds its file to Navigation and searches again", async () => {
    const backend = fakeBackend(MOVED);
    const session = await started(backend);
    session.selectLine(1);
    await vi.waitFor(() => expect(session.search).toBe("done"));

    await session.goDeeper();
    await vi.waitFor(() => expect(session.search).toBe("done"));

    expect(session.location).toEqual({ path: "donor.rs", rev: "c0", line: 2 });
    expect(session.items.filter((item) => item.kind === "header").map((item) => item.path)).toEqual([
      "main.rs",
      "donor.rs",
    ]);
    expect(backend.blame).toHaveBeenLastCalledWith("donor.rs", "c0", false);
    expect(backend.origins).toHaveBeenCalledTimes(2);
    expect(session.canGoBack).toBe(true);
  });

  it("goes back to where it was, line included", async () => {
    const backend = fakeBackend(MOVED);
    const session = await started(backend);
    session.selectLine(1);
    await vi.waitFor(() => expect(session.search).toBe("done"));
    await session.goDeeper();

    await session.back();

    expect(session.location).toEqual({ path: "main.rs", rev: null, line: 2 });
    expect(backend.blame).toHaveBeenLastCalledWith("main.rs", null, false);
    expect(session.canGoForward).toBe(true);
  });

  it("a newer pick cancels the search still running for the old one", async () => {
    const backend = fakeBackend(MOVED);
    let finish: (report: OriginReport | null) => void = () => {};
    backend.origins.mockImplementationOnce(
      (_query, onStarted) =>
        new Promise<OriginReport | null>((resolve) => {
          onStarted(41);
          finish = resolve;
        }),
    );
    const session = await started(backend);

    session.selectLine(0);
    session.selectLine(2);
    finish(MOVED);
    await vi.waitFor(() => expect(session.search).toBe("done"));

    expect(backend.cancel).toHaveBeenCalledWith(41);
    expect(backend.origins).toHaveBeenCalledTimes(2);
  });

  it("drops a blame that arrives after the user moved on", async () => {
    const backend = fakeBackend();
    const session = await started(backend);
    let late: (value: BlameTables) => void = () => {};
    backend.blame.mockImplementationOnce(
      () => new Promise<BlameTables>((resolve) => (late = resolve)),
    );

    const slow = session.selectItem(2);
    await session.selectItem(3);
    late(tables("stale", "main.rs"));
    await slow;

    expect(session.location.rev).toBe("c1");
    expect(session.blame?.commits[0]?.oid).toBe("c1");
  });

  it("steps to the older version in Navigation", async () => {
    const backend = fakeBackend();
    const session = await started(backend);

    await session.step(1);
    expect(session.location.rev).toBe("c2");
    await session.step(1);
    expect(session.location.rev).toBe("c1");
    await session.step(1);
    expect(session.location.rev, "the last row has nothing older").toBe("c1");
  });

  it("closes the card with the cross and has nothing to go deeper into without a target", async () => {
    const backend = fakeBackend({ candidates: [candidate("appeared", null)], best: 0 });
    const session = await started(backend);
    session.selectLine(0);
    await vi.waitFor(() => expect(session.search).toBe("done"));

    session.closeCard();
    await session.goDeeper();

    expect(session.cardOpen).toBe(false);
    expect(session.history.entries).toHaveLength(1);
  });
});

describe("going back while a blame is still loading", () => {
  // Back lands on the blame already on screen, so nothing is loaded — and the load for
  // the place just left used to finish afterwards and replace it.
  it("keeps the blame of the place it went back to", async () => {
    const backend = fakeBackend();
    const session = await started(backend);
    let deeper!: (tables: BlameTables) => void;
    backend.blame.mockReturnValueOnce(new Promise((resolve) => (deeper = resolve)));

    const going = session.navigate({ path: "donor.rs", rev: "c0^", line: null });
    await vi.waitFor(() => expect(backend.blame).toHaveBeenCalledTimes(2));
    await session.back();
    deeper(tables("c0", "donor.rs"));
    await going;

    expect(session.location.path).toBe("main.rs");
    expect(session.blameOf).toEqual({ path: "main.rs", rev: null });
    expect(session.blame?.commits[0]?.oid).toBe("wt");
    expect(session.loadingBlame).toBe(false);
  });
});
