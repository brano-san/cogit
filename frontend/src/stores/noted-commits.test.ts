import { beforeEach, describe, expect, it, vi } from "vitest";

const ipc = vi.hoisted(() => ({ notedCommits: vi.fn(), commitNotes: vi.fn() }));
vi.mock("$lib/ipc/ref-ops", () => ipc);

import { NotedCommits } from "./noted-commits.svelte";

beforeEach(() => vi.resetAllMocks());

describe("NotedCommits", () => {
  it("asks once for the whole set and answers per commit", async () => {
    ipc.notedCommits.mockResolvedValue(["a"]);
    const store = new NotedCommits();
    await store.refresh(1);
    expect(store.has("a")).toBe(true);
    expect(store.has("b")).toBe(false);
    expect(ipc.notedCommits).toHaveBeenCalledTimes(1);
  });

  it("loads a note's text once and forgets it on refresh", async () => {
    ipc.notedCommits.mockResolvedValue(["a"]);
    ipc.commitNotes.mockResolvedValue([{ namespace: "commits", text: "hi" }]);
    const store = new NotedCommits();
    await store.refresh(1);
    await store.load("a");
    await store.load("a");
    expect(store.texts.get("a")).toBe("hi");
    expect(ipc.commitNotes).toHaveBeenCalledTimes(1);
    await store.refresh(1);
    expect(store.texts.size).toBe(0);
  });

  it("drops the previous repository's set while the new one loads", async () => {
    ipc.notedCommits.mockResolvedValueOnce(["a"]).mockReturnValueOnce(new Promise(() => {}));
    const store = new NotedCommits();
    await store.refresh(1);
    void store.refresh(2);
    expect(store.has("a")).toBe(false);
  });
});
