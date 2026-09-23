import { beforeEach, describe, expect, it, vi } from "vitest";

class Channel {
  onmessage: ((message: unknown) => void) | null = null;
}

/** The Rust side: the rows of every graph built so far, by generation. */
const built = new Map<number, string[]>();
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
        complete: false,
        commits: oids.map((oid) => ({ oid })),
        rows: oids.map((_, i) => ({ row: start + i })),
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

const { graph } = await import("./graph.svelte");

type RepoId = import("$lib/ipc").RepoId;
const A = 1 as RepoId;
const B = 2 as RepoId;

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

function last() {
  const stream = streams.at(-1);
  if (!stream) throw new Error("no stream started");
  return {
    send: async (oids: string[], isLast = false) => {
      const rows = built.get(stream.generation)!;
      rows.push(...oids);
      stream.channel.onmessage?.({ generation: stream.generation, total: rows.length, isLast });
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

beforeEach(() => {
  graph.clear();
  streams.length = 0;
  built.clear();
  graph.show(0, 50);
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

  it("never shows one repository's history while another one loads", async () => {
    await loaded(A, ["a", "b"]);

    void graph.load(B);

    expect(oids()).toEqual([]);
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

  it("loads the row a key press moves to", async () => {
    await long();

    expect((await graph.entry(600))?.commit.oid).toBe("c600");
  });
});
