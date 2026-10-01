import { beforeEach, describe, expect, it, vi } from "vitest";

const saved = vi.hoisted(() => ({ value: undefined as unknown, writes: [] as unknown[] }));
vi.mock("$lib/settings-file", () => ({
  readKey: vi.fn(async () => saved.value),
  writeKey: vi.fn(async (_key: string, value: unknown) => {
    saved.writes.push(value);
  }),
}));

const seen = vi.hoisted(() => ({ labels: [] as string[] }));
vi.mock("$stores/network.svelte", () => ({
  network: {
    run: async (_repo: unknown, label: string, work: (onLine: (line: string) => void) => Promise<unknown>) => {
      seen.labels.push(label);
      return work(() => {});
    },
  },
}));
vi.mock("$lib/ipc/network-dialogs", () => ({
  pullWith: vi.fn(async () => ({ remote: "o", diverged: [] })),
  fetchWith: vi.fn(async () => ({ remote: "o", diverged: [] })),
  pushWith: vi.fn(async () => ({ notesRejected: null })),
  pushNotes: vi.fn(async () => ({ notesRejected: null })),
  mergeNotes: vi.fn(async () => {}),
}));

const { networkDialog, networkApi } = await import("./network-dialog.svelte");

beforeEach(() => {
  saved.value = undefined;
  saved.writes.length = 0;
  seen.labels.length = 0;
  networkDialog.moreOpen = false;
  networkDialog.close();
});

describe("More Options", () => {
  it("opens as the user left it", async () => {
    saved.value = { moreOpen: true };
    await networkDialog.load();
    expect(networkDialog.moreOpen).toBe(true);
  });

  it("is closed when nothing was saved", async () => {
    await networkDialog.load();
    expect(networkDialog.moreOpen).toBe(false);
  });

  it("is saved when toggled", async () => {
    await networkDialog.setMoreOpen(true);
    expect(networkDialog.moreOpen).toBe(true);
    expect(saved.writes).toEqual([{ moreOpen: true }]);
  });
});

describe("the dialogs' commands", () => {
  it("run as network operations the footer can name", async () => {
    await networkApi.pullWith(1, "o", { method: "merge", ffOnly: false, fetch: { tags: false, notes: false } });
    await networkApi.fetchWith(1, "o", { tags: false, notes: false });
    await networkApi.pushWith(1, {
      remote: "o",
      local: "a",
      branch: "a",
      setUpstream: false,
      tags: "none",
      notes: false,
      forceWithLease: false,
    });
    await networkApi.pushNotes(1, "o");
    expect(seen.labels).toEqual(["Pulling", "Fetching", "Pushing", "Pushing notes"]);
  });
});
