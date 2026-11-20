import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DiffSpec, FileEntry } from "$lib/ipc";
import type { BranchReview, LineCounts, ReviewPath } from "$lib/ipc/review";

vi.mock("$lib/ipc/review", () => ({
  reviewBase: vi.fn(),
  branchReview: vi.fn(),
  reviewCommitFiles: vi.fn(),
  reviewFiles: vi.fn(),
  reviewLineCounts: vi.fn(),
}));
vi.mock("$lib/ipc", () => ({
  toCogitError: (err: unknown) => ({ message: String(err) }),
}));

const { ReviewStore, COUNT_BATCH, NO_BASE, totalsOf } = await import("./review.svelte");

const REPO = 1 as never;

function file(path: string, extra: Partial<FileEntry> = {}): FileEntry {
  return { path, status: "added", mode: "plain", ...extra } as FileEntry;
}

function summary(extra: Partial<BranchReview> = {}): BranchReview {
  return {
    base: "main",
    baseOid: "b".repeat(40),
    branch: "topic",
    branchOid: "t".repeat(40),
    mergeBase: "m".repeat(40),
    mergeBaseSummary: "Shared work",
    mergeBases: ["m".repeat(40)],
    ahead: 2,
    behind: 1,
    commits: [
      { oid: "c2", summary: "two", author: { name: "a", email: "e", timestamp: 0, tzOffsetMinutes: 0 }, parents: 1 },
      { oid: "c1", summary: "one", author: { name: "a", email: "e", timestamp: 0, tzOffsetMinutes: 0 }, parents: 1 },
    ],
    isHead: true,
    ...extra,
  };
}

function fakeIpc(files: FileEntry[], chunk = 2) {
  return {
    reviewBase: vi.fn(async () => "main" as string | null),
    branchReview: vi.fn(async () => summary()),
    reviewFiles: vi.fn(
      async (_repo: unknown, _base: string, _branch: string, _worktree: boolean, onchunk: (files: FileEntry[]) => void) => {
        for (let at = 0; at < files.length; at += chunk) onchunk(files.slice(at, at + chunk));
        return files.length;
      },
    ),
    reviewLineCounts: vi.fn(async (_repo: unknown, _spec: DiffSpec, asked: ReviewPath[]): Promise<LineCounts[]> =>
      asked.map((each) => ({ path: each.path, added: 1, removed: 0 })),
    ),
    commitFiles: vi.fn(async (): Promise<FileEntry[]> => [file("b.txt")]),
  };
}

let show: ReturnType<typeof vi.fn<(repo: number, spec: DiffSpec, path: string) => void>>;

beforeEach(() => {
  show = vi.fn<(repo: number, spec: DiffSpec, path: string) => void>();
});

describe("opening a review", () => {
  it("finds the base, then lists the files chunk by chunk", async () => {
    const ipc = fakeIpc([file("a.txt"), file("b.txt"), file("c.txt")]);
    const store = new ReviewStore(show, ipc);

    await store.open(REPO, "topic", null);

    expect(ipc.reviewBase).toHaveBeenCalledWith(REPO, "topic");
    expect(store.base).toBe("main");
    expect(store.files.map((each) => each.path)).toEqual(["a.txt", "b.txt", "c.txt"]);
    expect(ipc.reviewFiles.mock.calls[0]?.slice(1, 4)).toEqual(["m".repeat(40), "t".repeat(40), false]);
  });

  it("says so when no main line is found", async () => {
    const ipc = fakeIpc([]);
    ipc.reviewBase.mockResolvedValueOnce(null);
    const store = new ReviewStore(show, ipc);

    await store.open(REPO, "topic", null);

    expect(store.problem).toBe(NO_BASE);
    expect(ipc.branchReview).not.toHaveBeenCalled();
  });

  it("keeps the refusal of unrelated histories as the error", async () => {
    const ipc = fakeIpc([]);
    ipc.branchReview.mockRejectedValueOnce(new Error("unrelated histories"));
    const store = new ReviewStore(show, ipc);

    await store.open(REPO, "topic", "main");

    expect(store.error?.message).toContain("unrelated histories");
    expect(store.summary).toBeNull();
    expect(ipc.reviewFiles).not.toHaveBeenCalled();
  });

  it("shows a deleted remote ref as an error on refresh", async () => {
    const ipc = fakeIpc([file("a.txt")]);
    const store = new ReviewStore(show, ipc);
    const branch = "refs/remotes/origin/topic";
    await store.open(REPO, branch, "main");
    ipc.branchReview.mockRejectedValueOnce(new Error(`cannot resolve ${branch}`));

    await store.reload();

    expect(store.summary).toBeNull();
    expect(store.shown).toEqual([]);
    expect(store.error?.message).toContain(branch);
  });

  it("calls a branch with nothing of its own merged", async () => {
    const ipc = fakeIpc([]);
    ipc.branchReview.mockResolvedValueOnce(summary({ ahead: 0, commits: [] }));
    const store = new ReviewStore(show, ipc);

    await store.open(REPO, "topic", "main");

    expect(store.merged).toBe(true);
  });

  it("notes a criss-cross history and names the base it diffs from", async () => {
    const ipc = fakeIpc([]);
    ipc.branchReview.mockResolvedValueOnce(summary({ mergeBases: ["m".repeat(40), "n".repeat(40)] }));
    const store = new ReviewStore(show, ipc);

    await store.open(REPO, "topic", "main");

    expect(store.note).toContain("2 merge bases");
    expect(store.note).toContain("mmmmmmm");
  });
});

describe("a large branch", () => {
  it("counts lines in batches in the background", async () => {
    const many = Array.from({ length: COUNT_BATCH * 2 + 5 }, (_, i) => file(`f${String(i).padStart(4, "0")}.txt`));
    const ipc = fakeIpc(many, 200);
    const store = new ReviewStore(show, ipc);

    await store.open(REPO, "topic", "main");

    expect(store.files).toHaveLength(many.length);
    expect(ipc.reviewFiles.mock.calls).toHaveLength(1);
    expect(ipc.reviewLineCounts.mock.calls.map((call) => call[2].length)).toEqual([COUNT_BATCH, COUNT_BATCH, 5]);
    expect(store.totals).toEqual({ added: many.length, removed: 0, counted: many.length });
  });

  it("stops counting for a load that is no longer current", async () => {
    const many = Array.from({ length: COUNT_BATCH * 3 }, (_, i) => file(`f${i}.txt`));
    const ipc = fakeIpc(many, 500);
    const store = new ReviewStore(show, ipc);
    ipc.reviewLineCounts.mockImplementationOnce(async (_repo, _spec, asked) => {
      void store.selectCommit("c1");
      return asked.map((each) => ({ path: each.path, added: 1, removed: 0 }));
    });

    await store.open(REPO, "topic", "main");
    await Promise.resolve();

    const allSpec = ipc.reviewLineCounts.mock.calls.filter((call) => call[1].kind === "commitVsCommit");
    expect(allSpec).toHaveLength(1);
  });

  it("skips submodules and asks with the old path of a rename", async () => {
    const ipc = fakeIpc([file("new.txt", { status: "renamed", oldPath: "old.txt" }), file("mod", { mode: "submodule" })]);
    const store = new ReviewStore(show, ipc);

    await store.open(REPO, "topic", "main");

    expect(ipc.reviewLineCounts.mock.calls[0]?.[2]).toEqual([{ path: "new.txt", oldPath: "old.txt" }]);
  });
});

describe("the commit filter", () => {
  it("lists one commit's files and shows them against its parent, a second click goes back", async () => {
    const ipc = fakeIpc([file("a.txt"), file("b.txt")]);
    const store = new ReviewStore(show, ipc);
    await store.open(REPO, "topic", "main");

    await store.selectCommit("c1");
    expect(store.shown.map((each) => each.path)).toEqual(["b.txt"]);
    store.select("b.txt");
    expect(show).toHaveBeenLastCalledWith(REPO, { kind: "commitVsParent", oid: "c1" }, "b.txt");

    await store.selectCommit("c1");
    expect(store.commit).toBeNull();
    expect(store.shown.map((each) => each.path)).toEqual(["a.txt", "b.txt"]);
    store.select("b.txt");
    expect(show).toHaveBeenLastCalledWith(REPO, { kind: "commitVsCommit", a: "m".repeat(40), b: "t".repeat(40) }, "b.txt");
  });

  it("drops a selected file the filter does not list", async () => {
    const ipc = fakeIpc([file("a.txt"), file("b.txt")]);
    const store = new ReviewStore(show, ipc);
    await store.open(REPO, "topic", "main");
    store.select("a.txt");

    await store.selectCommit("c1");

    expect(store.selected).toBeNull();
  });
});

describe("the working tree", () => {
  it("joins only the checked-out branch, and diffs renames between commits", async () => {
    const ipc = fakeIpc([file("x.txt"), file("new.txt", { status: "renamed", oldPath: "old.txt" })]);
    const store = new ReviewStore(show, ipc);
    await store.open(REPO, "topic", "main");

    await store.setWorktree(true);

    expect(ipc.reviewFiles.mock.calls.at(-1)?.[3]).toBe(true);
    expect(store.specFor(store.files[0]!)).toEqual({ kind: "commitVsWorkTree", oid: "m".repeat(40) });
    expect(store.specFor(store.files[1]!)).toEqual({ kind: "commitVsCommit", a: "m".repeat(40), b: "t".repeat(40) });
  });

  it("is refused for a branch that is not checked out", async () => {
    const ipc = fakeIpc([]);
    ipc.branchReview.mockResolvedValueOnce(summary({ isHead: false }));
    const store = new ReviewStore(show, ipc);
    await store.open(REPO, "topic", "main");

    await store.setWorktree(true);

    expect(store.worktree).toBe(false);
    expect(ipc.reviewFiles).toHaveBeenCalledTimes(1);
  });

  it("keeps working-tree counts visible until the new counts arrive", async () => {
    const entry = file("x.txt");
    const ipc = fakeIpc([entry]);
    const store = new ReviewStore(show, ipc);
    await store.open(REPO, "topic", "main");
    await store.setWorktree(true);
    const pending = Promise.withResolvers<LineCounts[]>();
    const started = Promise.withResolvers<void>();
    ipc.reviewLineCounts.mockImplementationOnce(() => {
      started.resolve();
      return pending.promise;
    });

    const refresh = store.reload();
    await started.promise;
    expect(store.countsOf(entry)?.added).toBe(1);
    pending.resolve([{ path: entry.path, added: 3, removed: 0 }]);
    await refresh;
    expect(store.countsOf(entry)?.added).toBe(3);
  });
});

describe("totals", () => {
  it("leave binary files out of the sums but count them as counted", () => {
    expect(
      totalsOf([
        { path: "a", added: 3, removed: 1 },
        { path: "b", added: null, removed: null },
        undefined,
      ]),
    ).toEqual({ added: 3, removed: 1, counted: 2 });
  });
});

describe("the open file", () => {
  it("refreshes a selected file against the new tip while keeping the selection", async () => {
    const files = [file("a.txt"), file("b.txt")];
    const ipc = fakeIpc(files);
    const store = new ReviewStore(show, ipc);
    await store.open(REPO, "topic", "main");
    store.select("b.txt");
    ipc.branchReview.mockResolvedValueOnce(summary({ branchOid: "next-tip" }));

    await store.reload();

    expect(store.selected).toBe("b.txt");
    expect(ipc.reviewFiles.mock.lastCall?.[2]).toBe("next-tip");
    expect(show).toHaveBeenLastCalledWith(REPO, { kind: "commitVsCommit", a: "m".repeat(40), b: "next-tip" }, "b.txt");
  });

  it("keeps a selected file while refreshing a commit filter", async () => {
    const ipc = fakeIpc([file("a.txt")]);
    const store = new ReviewStore(show, ipc);
    await store.open(REPO, "topic", "main");
    await store.selectCommit("c1");
    store.select("b.txt");

    await store.reload();

    expect(store.selected).toBe("b.txt");
  });

  it("changes the base without keeping the previous commit filter", async () => {
    const ipc = fakeIpc([file("b.txt")]);
    const store = new ReviewStore(show, ipc);
    await store.open(REPO, "topic", "main");
    await store.selectCommit("c1");
    store.select("b.txt");
    ipc.branchReview.mockResolvedValueOnce(summary({ base: "release", mergeBase: "alternate" }));

    await store.setBase("release");

    expect(ipc.branchReview).toHaveBeenLastCalledWith(REPO, "release", "topic");
    expect(store.commit).toBeNull();
    expect(store.selected).toBe("b.txt");
    expect(show).toHaveBeenLastCalledWith(REPO, { kind: "commitVsCommit", a: "alternate", b: "t".repeat(40) }, "b.txt");
  });

  it("keeps the chosen base when an older default lookup completes", async () => {
    const ipc = fakeIpc([file("b.txt")]);
    const pending = Promise.withResolvers<string | null>();
    ipc.reviewBase.mockReturnValueOnce(pending.promise);
    const store = new ReviewStore(show, ipc);
    const opening = store.open(REPO, "topic", null);

    await store.setBase("release");
    pending.resolve("main");
    await opening;

    expect(store.base).toBe("release");
  });

  it("loads the whole list when returning from a filter selected during streaming", async () => {
    const ipc = fakeIpc([file("a.txt"), file("b.txt")]);
    const pending = Promise.withResolvers<void>();
    const started = Promise.withResolvers<void>();
    ipc.reviewFiles.mockImplementationOnce(async (_repo, _base, _branch, _worktree, onchunk) => {
      onchunk([file("a.txt")]);
      started.resolve();
      await pending.promise;
      onchunk([file("b.txt")]);
      return 2;
    });
    const store = new ReviewStore(show, ipc);
    const opening = store.open(REPO, "topic", "main");
    await started.promise;
    await store.selectCommit("c1");
    pending.resolve();
    await opening;

    await store.selectCommit(null);

    expect(store.shown.map((each) => each.path)).toEqual(["a.txt", "b.txt"]);
  });

  it("follows the filter: the commit's own change of it is shown", async () => {
    const ipc = fakeIpc([file("a.txt"), file("b.txt")]);
    const store = new ReviewStore(show, ipc);
    await store.open(REPO, "topic", "main");
    store.select("b.txt");

    await store.selectCommit("c1");

    expect(store.selected).toBe("b.txt");
    expect(show).toHaveBeenLastCalledWith(REPO, { kind: "commitVsParent", oid: "c1" }, "b.txt");
  });
});
