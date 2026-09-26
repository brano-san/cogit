import { beforeEach, describe, expect, it, vi } from "vitest";

class Channel {
  onmessage: ((message: unknown) => void) | null = null;
}

/** The Rust side: the rows of every graph built so far, by generation. */
const built = new Map<number, string[]>();
const walked = new Set<number>();
let generations = 0;
const streams: { channel: Channel; generation: number; finish: (outcome: unknown) => void }[] = [];

const commands = {
  loadCommits: vi.fn(
    (_repo: number, _query: unknown, channel: Channel) =>
      new Promise((resolve) => {
        const generation = ++generations;
        built.set(generation, []);
        streams.push({ channel, generation, finish: resolve });
      }),
  ),
  graphWindow: vi.fn(async (_repo: number, generation: number, start: number, count: number) => {
    const rows = generation === generations ? built.get(generation) : undefined;
    if (!rows) return { status: "ok", data: null };
    const oids = rows.slice(start, start + count);
    return {
      status: "ok",
      data: {
        start,
        total: rows.length,
        complete: walked.has(generation),
        length: oids.length,
        oid: (row: number) => oids[row],
        find: (oid: string) => oids.indexOf(oid),
        entry: (row: number) => ({ commit: { oid: oids[row] }, layout: { row: start + row, lane: (start + row) % 3 } }),
      },
    };
  }),
  graphRowOf: vi.fn(async (_repo: number, generation: number, oid: string) => {
    const at = generation === generations ? (built.get(generation)?.indexOf(oid) ?? -1) : -1;
    return { status: "ok", data: at < 0 ? null : at };
  }),
};

vi.mock("@tauri-apps/api/core", () => ({ Channel }));
vi.mock("$lib/ipc/bindings", () => ({ commands }));
vi.mock("$lib/graph-wire", () => ({ decodeBase64Window: (block: unknown) => block }));

const { graph } = await import("./graph.svelte");
const { repository } = await import("./repository.svelte");

type RepoId = import("$lib/ipc").RepoId;
const A = 1 as RepoId;
const B = 2 as RepoId;

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

function last() {
  const stream = streams.at(-1);
  if (!stream) throw new Error("no stream started");
  /** Rows laid out in Rust, and whether the walk is over, without a word to the channel. */
  const walk = (oids: string[], isLast: boolean) => {
    const rows = built.get(stream.generation)!;
    rows.push(...oids);
    if (isLast) walked.add(stream.generation);
    return rows.length;
  };
  /** A progress message; the rows a window asks for meanwhile are still on their way. */
  const push = (oids: string[], isLast = false) => {
    const total = walk(oids, isLast);
    stream.channel.onmessage?.({ generation: stream.generation, total, isLast });
  };
  return {
    walk,
    push,
    send: async (oids: string[], isLast = false) => {
      push(oids, isLast);
      await settle();
    },
    finish: () => stream.finish({ status: "ok", data: [] }),
    fail: () => stream.finish({ status: "error", error: { kind: "internal", data: "boom" } }),
  };
}

/** What the list shows: the rows it asked for, as far as they have arrived. */
function oids() {
  const shown: string[] = [];
  for (let i = 0; i < graph.total; i++) {
    const oid = graph.rowAt(i)?.commit.oid;
    if (oid) shown.push(oid);
  }
  return shown;
}

async function loaded(repo: RepoId, history: string[]) {
  const done = graph.load(repo);
  await last().send(history, true);
  last().finish();
  await done;
  graph.show(0, graph.total);
  await settle();
}

/** The panels move on to `repo`, as they do before its graph is asked for. */
const open = (repo: RepoId) =>
  repository.adopt({ repo, root: `/${repo}` } as import("$lib/ipc").RepoSummary);

beforeEach(() => {
  repository.phase = { kind: "open", repo: { repo: A, root: `/${A}` } as import("$lib/ipc").RepoSummary };
  graph.clear();
  streams.length = 0;
  built.clear();
  walked.clear();
  graph.show(0, 50);
});

const ids = (count: number, prefix: string) => Array.from({ length: count }, (_, i) => `${prefix}${i}`);

describe("graph counter", () => {
  it("settles on the newest walk after reloads restarted in a row", async () => {
    await loaded(A, ["a", "b", "c"]);

    void graph.load(A);
    const first = last();
    void graph.load(A);
    const second = last();
    const done = graph.load(A);
    const third = last();
    await first.send(["x"], true);
    await second.send(["y", "z"], true);
    await third.send(["p", "q", "r", "s"], true);
    third.finish();
    await done;

    expect(graph.total).toBe(4);
    expect(oids()).toEqual(["p", "q", "r", "s"]);
  });

  it("keeps the old count with the old rows while a restarted reload has nothing yet", async () => {
    await loaded(A, ["a", "b", "c"]);

    void graph.load(A);
    void graph.load(A);

    expect(graph.total).toBe(3);
    expect(oids()).toEqual(["a", "b", "c"]);
  });

  it("finishes a reload whose rows were on their way when its last count arrived", async () => {
    await loaded(A, ids(300, "o"));
    graph.show(100, 200);
    await settle();

    const reload = graph.load(A);
    const stream = last();
    stream.push(ids(200, "n"));
    stream.push(ids(150, "m"), true);
    await settle();
    stream.finish();
    await reload;
    await settle();

    expect(graph.total).toBe(350);
    expect(graph.rowAt(150)?.commit.oid).toBe("n150");
  });

  it("takes the final count from Rust when the last progress message never came", async () => {
    await loaded(A, ["a", "b", "c"]);

    const reload = graph.load(A);
    const stream = last();
    await stream.send(ids(200, "n"));
    stream.walk(ids(150, "m"), true);
    stream.finish();
    await reload;
    await settle();

    expect(graph.total).toBe(350);
    expect(graph.complete).toBe(true);
  });

  it("walks again when a walk it does not know about cut the newest one short", async () => {
    await loaded(A, ["a", "b", "c"]);

    const reload = graph.load(A);
    const started = streams.length;
    const cut = last();
    generations += 1;
    cut.finish();
    await settle();
    expect(streams.length).toBe(started + 1);
    const again = last();
    await again.send(["x", "y"], true);
    again.finish();
    await reload;
    await settle();

    expect(graph.total).toBe(2);
    expect(oids()).toEqual(["x", "y"]);
  });
});

describe("graph reload", () => {
  it("keeps the history on screen until the new one has caught up with it", async () => {
    await loaded(A, ["a", "b", "c"]);

    const reload = graph.load(A);
    expect(oids()).toEqual(["a", "b", "c"]);
    await last().send(["x", "y"]);
    expect(oids()).toEqual(["a", "b", "c"]);
    await last().send(["z", "w"]);
    expect(oids()).toEqual(["x", "y", "z", "w"]);
    await last().send(["v"], true);
    last().finish();
    await reload;
    graph.show(0, graph.total);
    await settle();

    expect(oids()).toEqual(["x", "y", "z", "w", "v"]);
  });

  /** A reload whose first `kept` rows Rust says are the rows of generation `base`. */
  async function reloadKeeping(rows: string[], base: number, kept: number) {
    const reload = graph.load(A);
    const stream = streams.at(-1)!;
    built.get(stream.generation)!.push(...rows);
    walked.add(stream.generation);
    stream.channel.onmessage?.({ generation: stream.generation, total: rows.length, isLast: true, base, kept });
    await settle();
    stream.finish({ status: "ok", data: [] });
    await reload;
    return commands.graphWindow.mock.calls
      .filter(([, generation]) => generation === stream.generation)
      .map(([, , start]) => start);
  }

  it("keeps the blocks a reload repeats row for row instead of asking for them", async () => {
    const old = ids(300, "o");
    await loaded(A, old);
    const shown = streams.at(-1)!.generation;
    commands.graphWindow.mockClear();

    const asked = await reloadKeeping([...old.slice(0, 200), ...ids(100, "n")], shown, 200);

    expect(asked).not.toContain(0);
    expect(asked).toContain(128);
    expect(graph.rowAt(0)?.commit.oid).toBe("o0");
    expect(graph.rowAt(250)?.commit.oid).toBe("n50");
  });

  it("asks for every block when the rows kept belong to another walk", async () => {
    await loaded(A, ids(300, "o"));
    const shown = streams.at(-1)!.generation;
    commands.graphWindow.mockClear();

    const asked = await reloadKeeping(ids(300, "n"), shown - 1, 300);

    expect(asked).toContain(0);
    expect(graph.rowAt(0)?.commit.oid).toBe("n0");
  });

  it("swaps in a shorter history once it is complete", async () => {
    await loaded(A, ["a", "b", "c"]);

    const reload = graph.load(A);
    await last().send(["x"], true);
    last().finish();
    await reload;

    expect(oids()).toEqual(["x"]);
  });

  it("empties the list when the new history has nothing in it", async () => {
    await loaded(A, ["a", "b"]);

    const reload = graph.load(A);
    await last().send([], true);
    last().finish();
    await reload;

    expect(oids()).toEqual([]);
  });

  // Switching blinked: A's graph, an empty list, then B's (R-300).
  it("keeps the last repository's history on screen until the next one's arrives", async () => {
    await loaded(A, ["a", "b"]);

    open(B);
    const load = graph.load(B);
    expect(oids()).toEqual(["a", "b"]);
    expect(graph.shownRepo).toBe(A);
    await last().send(["x", "y", "z"], true);
    last().finish();
    await load;

    expect(oids()).toEqual(["x", "y", "z"]);
    expect(graph.shownRepo).toBe(B);
  });

  it("opens another repository's history at its top, wherever the last one was", async () => {
    await loaded(A, ids(600, "a"));
    graph.show(500, 540);
    await settle();
    const home = graph.home;

    open(B);
    void graph.load(B);
    await last().send(ids(40, "b"));

    expect(graph.shownRepo).toBe(B);
    expect(graph.home).toBe(home + 1);
    expect(graph.rowAt(0)?.commit.oid).toBe("b0");
  });

  it("drops a load asked for by a repository the panels have left", async () => {
    await loaded(A, ["a", "b"]);
    open(B);
    const started = streams.length;

    await graph.load(A);

    expect(streams.length).toBe(started);
    expect(oids()).toEqual(["a", "b"]);
  });

  it("never lets a reload of the repository left take the screen after it", async () => {
    await loaded(A, ["a", "b"]);
    void graph.load(A);
    const reload = last();

    open(B);
    await reload.send(["x", "y", "z"], true);

    expect(oids()).toEqual(["a", "b"]);
    expect(graph.loading).toBe(false);
  });

  it("shows a first history as it streams in", async () => {
    void graph.load(A);
    await last().send(["a", "b"]);

    expect(oids()).toEqual(["a", "b"]);
  });

  it("drops the old rows when the reload fails", async () => {
    await loaded(A, ["a", "b"]);

    const reload = graph.load(A);
    last().fail();
    await reload;

    expect(graph.error).not.toBeNull();
    expect(oids()).toEqual([]);
  });

  it("ignores what a superseded reload still sends", async () => {
    await loaded(A, ["a", "b"]);

    void graph.load(A);
    const stale = last();
    void graph.load(A);
    await stale.send(["old1", "old2", "old3"], true);

    expect(oids()).toEqual(["a", "b"]);
  });
});

describe("graph windows", () => {
  const history = Array.from({ length: 1000 }, (_, i) => `c${i}`);

  async function long() {
    graph.show(0, 40);
    void graph.load(A);
    await last().send(history, true);
  }

  it("asks only for the rows on screen", async () => {
    commands.graphWindow.mockClear();
    await long();

    expect(graph.total).toBe(1000);
    expect(graph.rowAt(10)?.commit.oid).toBe("c10");
    expect(graph.rowAt(900)).toBeUndefined();
    const ends = commands.graphWindow.mock.calls.map(([, , start, count]) => start + count);
    expect(Math.max(...ends)).toBeLessThanOrEqual(256);
  });

  it("fetches the rows a scroll brings on screen", async () => {
    await long();

    graph.show(700, 740);
    await settle();

    expect(graph.rowAt(720)?.commit.oid).toBe("c720");
  });

  it("finds a commit that is not loaded by asking for its row", async () => {
    await long();

    expect(await graph.indexOf("c10")).toBe(10);
    expect(await graph.indexOf("c950")).toBe(950);
    expect(await graph.indexOf("nowhere")).toBeNull();
  });

  it("places a commit far off the screen without keeping its block", async () => {
    await long();

    expect(await graph.locate("c950")).toEqual({ row: 950, lane: 950 % 3 });
    expect(graph.rowAt(950)).toBeUndefined();
    expect(await graph.locate("c10")).toEqual({ row: 10, lane: 10 % 3 });
    expect(await graph.locate("nowhere")).toBeNull();
  });

  it("loads the row a key press moves to", async () => {
    await long();

    expect((await graph.entry(600))?.commit.oid).toBe("c600");
  });

  // PageDown held down: the scroll had already asked for the block, and the key press
  // asking again got nothing back, so the selection stayed while the list moved on.
  it("loads the row a key press moves to while a scroll is already fetching it", async () => {
    await long();

    graph.show(700, 740);

    expect((await graph.entry(720))?.commit.oid).toBe("c720");
  });
});

describe("first parents only", () => {
  const firstParent = (on: boolean) => ({ firstParent: on, collapseMerged: false, expanded: [] });
  const view = graph.view;

  it("asks the walk for first parents while the setting is on", async () => {
    commands.loadCommits.mockClear();
    graph.view = firstParent(true);
    void graph.load(A);
    graph.view = firstParent(false);
    void graph.load(A);
    graph.view = view;

    const asked = commands.loadCommits.mock.calls.map(
      ([, query]) => (query as { view: { firstParent: boolean } }).view.firstParent,
    );
    expect(asked).toEqual([true, false]);
  });

  it("walks the history on screen again when the setting changes, and only then", async () => {
    await loaded(A, ["a", "b"]);
    const started = streams.length;

    graph.setView(firstParent(true));
    graph.setView(firstParent(true));

    expect(streams.length).toBe(started + 1);
    graph.view = view;
  });
});

// Closing the last repository clears the graph; the "Not in the graph: …" line of the
// repository just closed stayed above the start screen.
describe("clearing the graph", () => {
  it("drops the refs the last walk skipped", async () => {
    const done = graph.load(A);
    const stream = last();
    await stream.send(["a"], true);
    streams.at(-1)!.finish({ status: "ok", data: [{ name: "refs/heads/odd", reason: "x" }] });
    await done;
    expect(graph.skipped.length).toBe(1);

    graph.clear();

    expect(graph.skipped).toEqual([]);
  });
});

describe("long links", () => {
  const sent = () => commands.loadCommits.mock.calls.at(-1)?.[1] as { longLinkRows?: number } | undefined;

  it("asks for them to be cut with every load", async () => {
    await loaded(A, ["a"]);

    expect(sent()?.longLinkRows).toBe(graph.longLinkRows);
  });

  it("lays the history out again when the threshold changes, and only then", async () => {
    await loaded(A, ["a", "b"]);
    const before = commands.loadCommits.mock.calls.length;

    graph.setLongLinkRows(graph.longLinkRows);
    expect(commands.loadCommits.mock.calls.length).toBe(before);

    graph.setLongLinkRows(0);
    expect(commands.loadCommits.mock.calls.length).toBe(before + 1);
    expect(sent()?.longLinkRows).toBe(0);
    graph.setLongLinkRows(40);
  });
});

// Switching to a large repository and changing a graph mode before its graph came walked
// the repository left, and that load was dropped: the new graph came without the mode.
describe("a setting changed while the next repository's graph loads", () => {
  const view = graph.view;
  const rows = graph.longLinkRows;

  async function switching() {
    await loaded(A, ["a", "b"]);
    open(B);
    void graph.load(B);
    commands.loadCommits.mockClear();
  }
  const walked = () => commands.loadCommits.mock.calls.map(([repo]) => repo);

  it("walks that repository again for another view", async () => {
    await switching();

    graph.setView({ firstParent: true, collapseMerged: false, expanded: [] });

    expect(walked()).toEqual([B]);
    graph.view = view;
  });

  it("walks that repository again for another long-link threshold", async () => {
    await switching();

    graph.setLongLinkRows(rows + 1);

    expect(walked()).toEqual([B]);
    graph.setLongLinkRows(rows);
  });
});

// Ctrl+W right after opening a large repository: Rust dropped the graph with the repository,
// the first load walked it again and reported "Could not load the graph" for a closed one.
describe("a repository closed while its first graph loads", () => {
  it("is not walked again, and its load reports nothing", async () => {
    const load = graph.load(A);
    const stream = last();
    const { generation } = streams.at(-1)!;
    await stream.send(["a"]);
    const started = streams.length;

    repository.close();
    built.delete(generation);
    stream.finish();
    await load;
    await settle();

    expect(streams.length).toBe(started);
    expect(graph.error).toBeNull();
    expect(graph.loading).toBe(false);
  });

  it("is not walked for work that finishes after it closed", async () => {
    repository.close();

    await graph.load(A);

    expect(streams.length).toBe(0);
  });
});
