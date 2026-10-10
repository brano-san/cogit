import { describe, expect, it, vi } from "vitest";
import { STASH_AND_PULL, pullFlow, pushFlow, resolveDivergedNotes, type FlowUi, type NetworkApi } from "./network-flow";

const none = { remote: "origin", diverged: [] as string[] };
const pushOptions = {
  remote: "origin",
  local: "a",
  branch: "a",
  setUpstream: false,
  tags: "none",
  notes: true,
  forceWithLease: false,
} as const;

function fakes(over: Partial<NetworkApi> = {}, answer = true) {
  const api: NetworkApi = {
    pullWith: vi.fn(async () => ({ notes: none, stashNeeded: false })),
    fetchWith: vi.fn(async () => none),
    pushWith: vi.fn(async () => ({ notesRejected: null })),
    pushNotes: vi.fn(async () => ({ notesRejected: null })),
    mergeNotes: vi.fn(async () => {}),
    ...over,
  };
  const ui: FlowUi = { ask: vi.fn(async () => answer), report: vi.fn() };
  return { api, ui };
}

describe("diverged notes", () => {
  it("are not asked about when nothing diverged", async () => {
    const { api, ui } = fakes();
    expect(await resolveDivergedNotes(api, ui, 1, none)).toBe(false);
    expect(ui.ask).not.toHaveBeenCalled();
  });

  it("are merged one namespace at a time on a yes", async () => {
    const { api, ui } = fakes();
    await resolveDivergedNotes(api, ui, 1, { remote: "origin", diverged: ["commits", "review"] });
    expect(api.mergeNotes).toHaveBeenNthCalledWith(1, 1, "origin", "commits");
    expect(api.mergeNotes).toHaveBeenNthCalledWith(2, 1, "origin", "review");
  });

  it("are left alone on a no", async () => {
    const { api, ui } = fakes({}, false);
    await resolveDivergedNotes(api, ui, 1, { remote: "origin", diverged: ["commits"] });
    expect(api.mergeNotes).not.toHaveBeenCalled();
  });
});

describe("pull and fetch only", () => {
  it("Pull goes through pull, Fetch Only through fetch", async () => {
    const { api, ui } = fakes();
    await pullFlow(api, ui, 1, "origin", { method: "merge", ffOnly: false, fetch: { tags: false, notes: true }, autostash: false });
    await pullFlow(api, ui, 1, "origin", { fetchOnly: { tags: true, notes: true } });
    expect(api.pullWith).toHaveBeenCalledTimes(1);
    expect(api.fetchWith).toHaveBeenCalledWith(1, "origin", { tags: true, notes: true });
  });

  it("offers the merge for what the pull found", async () => {
    const { api, ui } = fakes({ pullWith: vi.fn(async () => ({ notes: { remote: "origin", diverged: ["commits"] }, stashNeeded: false })) });
    await pullFlow(api, ui, 1, "origin", { method: "rebase", ffOnly: false, fetch: { tags: false, notes: true }, autostash: false });
    expect(api.mergeNotes).toHaveBeenCalledWith(1, "origin", "commits");
  });

  it("asks before stashing local changes in the way, then pulls with them stashed", async () => {
    const pullWith = vi
      .fn()
      .mockResolvedValueOnce({ notes: none, stashNeeded: true })
      .mockResolvedValueOnce({ notes: none, stashNeeded: false });
    const { api, ui } = fakes({ pullWith });
    const options = { method: "rebase", ffOnly: false, fetch: { tags: false, notes: false }, autostash: false } as const;

    expect(await pullFlow(api, ui, 1, "origin", options)).toBe(true);

    expect(ui.ask).toHaveBeenCalledWith(STASH_AND_PULL);
    expect(pullWith).toHaveBeenLastCalledWith(1, "origin", { ...options, autostash: true });
  });

  it("pulls nothing more when the local changes stay where they are", async () => {
    const pullWith = vi.fn(async () => ({ notes: none, stashNeeded: true }));
    const { api, ui } = fakes({ pullWith }, false);

    expect(
      await pullFlow(api, ui, 1, "origin", { method: "rebase", ffOnly: false, fetch: { tags: false, notes: false }, autostash: false }),
    ).toBe(false);
    expect(pullWith).toHaveBeenCalledTimes(1);
  });

  it("lets a failing pull fail without asking anything", async () => {
    const { api, ui } = fakes({ pullWith: vi.fn(async () => Promise.reject(new Error("conflict"))) });
    await expect(
      pullFlow(api, ui, 1, "origin", { method: "merge", ffOnly: false, fetch: { tags: false, notes: true }, autostash: false }),
    ).rejects.toThrow("conflict");
    expect(ui.ask).not.toHaveBeenCalled();
  });
});

describe("push", () => {
  it("does nothing more when the notes went", async () => {
    const { api, ui } = fakes();
    await pushFlow(api, ui, 1, pushOptions);
    expect(ui.ask).not.toHaveBeenCalled();
    expect(api.pushNotes).not.toHaveBeenCalled();
  });

  it("offers fetch, merge and a second push of the notes when they were refused", async () => {
    const { api, ui } = fakes({
      pushWith: vi.fn(async () => ({ notesRejected: "! [rejected] refs/notes/commits" })),
      fetchWith: vi.fn(async () => ({ remote: "origin", diverged: ["commits"] })),
    });
    await pushFlow(api, ui, 1, pushOptions);
    expect(vi.mocked(ui.ask).mock.calls[0]![0].message).toContain("! [rejected] refs/notes/commits");
    expect(api.fetchWith).toHaveBeenCalledWith(1, "origin", { tags: false, notes: true });
    expect(api.mergeNotes).toHaveBeenCalledWith(1, "origin", "commits");
    expect(api.pushNotes).toHaveBeenCalledWith(1, "origin");
  });

  it("stops at a no", async () => {
    const { api, ui } = fakes({ pushWith: vi.fn(async () => ({ notesRejected: "no" })) }, false);
    await pushFlow(api, ui, 1, pushOptions);
    expect(api.fetchWith).not.toHaveBeenCalled();
  });

  it("shows a second refusal in git's own words", async () => {
    const { api, ui } = fakes({
      pushWith: vi.fn(async () => ({ notesRejected: "first" })),
      pushNotes: vi.fn(async () => ({ notesRejected: "second" })),
    });
    await pushFlow(api, ui, 1, pushOptions);
    expect(ui.report).toHaveBeenCalledWith("second", "Could not push notes");
  });
});
