import { describe, expect, it, vi } from "vitest";

class Channel {
  onmessage: ((message: unknown) => void) | null = null;
}

const queries: unknown[] = [];
const commands = {
  loadCommits: vi.fn(async (_repo: number, query: unknown, channel: Channel) => {
    queries.push(query);
    channel.onmessage?.({ generation: queries.length, total: 0, isLast: true });
    return { status: "ok", data: [] };
  }),
  graphWindow: vi.fn(async () => ({ status: "ok", data: null })),
};

vi.mock("@tauri-apps/api/core", () => ({ Channel }));
vi.mock("$lib/ipc/bindings", () => ({ commands }));
vi.mock("$lib/graph-wire", () => ({ decodeBase64Window: (block: unknown) => block }));

const { graph } = await import("./graph.svelte");
const REPO = 1 as import("$lib/ipc").RepoId;

describe("graph view", () => {
  it("goes with every load and walks the graph again when it changes", async () => {
    await graph.load(REPO);
    expect(queries.at(-1)).toMatchObject({ view: { firstParent: false } });

    graph.setView({ firstParent: true });
    await vi.waitFor(() => expect(queries).toHaveLength(2));
    expect(queries.at(-1)).toMatchObject({ view: { firstParent: true } });

    graph.setView({ firstParent: true });
    await Promise.resolve();
    expect(queries).toHaveLength(2);
  });
});
