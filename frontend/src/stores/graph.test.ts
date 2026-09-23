import { beforeEach, describe, expect, it, vi } from "vitest";

class Channel {
  onmessage: ((chunk: unknown) => void) | null = null;
}

const streams: { channel: Channel; finish: (outcome: unknown) => void }[] = [];
const commands = {
  loadCommits: vi.fn(
    (_repo: number, _query: unknown, channel: Channel) =>
      new Promise((resolve) => streams.push({ channel, finish: resolve })),
  ),
};

vi.mock("@tauri-apps/api/core", () => ({ Channel }));
vi.mock("$lib/ipc/bindings", () => ({ commands }));

const { graph } = await import("./graph.svelte");

type RepoId = import("$lib/ipc").RepoId;
const A = 1 as RepoId;
const B = 2 as RepoId;

function chunk(oids: string[], isLast: boolean) {
  return {
    commits: oids.map((oid) => ({ oid })),
    rows: oids.map((_, row) => ({ row })),
    isLast,
  };
}

function last() {
  const stream = streams.at(-1);
  if (!stream) throw new Error("no stream started");
  return {
    send: (oids: string[], isLast = false) => stream.channel.onmessage?.(chunk(oids, isLast)),
    finish: () => stream.finish({ status: "ok", data: [] }),
    fail: () => stream.finish({ status: "error", error: { kind: "internal", data: "boom" } }),
  };
}

const oids = () => graph.rows.map((row) => row.commit.oid);

async function loaded(repo: RepoId, history: string[]) {
  const done = graph.load(repo);
  last().send(history, true);
  last().finish();
  await done;
}

beforeEach(() => {
  graph.clear();
  streams.length = 0;
});

describe("graph reload", () => {
  it("keeps the history on screen until the new one has caught up with it", async () => {
    await loaded(A, ["a", "b", "c"]);

    const reload = graph.load(A);
    expect(oids()).toEqual(["a", "b", "c"]);
    last().send(["x", "y"]);
    expect(oids()).toEqual(["a", "b", "c"]);
    last().send(["z", "w"]);
    expect(oids()).toEqual(["x", "y", "z", "w"]);
    last().send(["v"], true);
    last().finish();
    await reload;

    expect(oids()).toEqual(["x", "y", "z", "w", "v"]);
  });

  it("swaps in a shorter history once it is complete", async () => {
    await loaded(A, ["a", "b", "c"]);

    const reload = graph.load(A);
    last().send(["x"], true);
    last().finish();
    await reload;

    expect(oids()).toEqual(["x"]);
  });

  it("empties the list when the new history has nothing in it", async () => {
    await loaded(A, ["a", "b"]);

    const reload = graph.load(A);
    last().send([], true);
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
    last().send(["a", "b"]);

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
    stale.send(["old1", "old2", "old3"], true);

    expect(oids()).toEqual(["a", "b"]);
  });
});
