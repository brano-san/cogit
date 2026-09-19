import { beforeEach, describe, expect, it, vi } from "vitest";

const commands = {
  commitDetails: vi.fn(),
  commitFiles: vi.fn(),
};

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands }));

const { commit } = await import("./commit.svelte");

const REPO = { 0: 1 } as unknown as import("$lib/ipc").RepoId;

function ok<T>(data: T) {
  return { status: "ok" as const, data };
}

function details(oid: string) {
  return ok({
    oid,
    parents: [],
    summary: `summary of ${oid}`,
    body: "",
    author: { name: "A", email: "a@b", timestamp: 0, tzOffsetMinutes: 0 },
    committer: { name: "A", email: "a@b", timestamp: 0, tzOffsetMinutes: 0 },
  });
}

function files(path: string) {
  return ok([{ path, oldPath: null, status: "added" as const }]);
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

describe("commit store", () => {
  beforeEach(() => {
    commands.commitDetails.mockReset();
    commands.commitFiles.mockReset();
    commit.clear();
  });

  it("loads details and files for the selected commit", async () => {
    commands.commitDetails.mockResolvedValue(details("abc"));
    commands.commitFiles.mockResolvedValue(files("one.txt"));

    await commit.select(REPO, "abc");

    expect(commit.oid).toBe("abc");
    expect(commit.details?.summary).toBe("summary of abc");
    expect(commit.files.map((f) => f.path)).toEqual(["one.txt"]);
    expect(commit.loading).toBe(false);
    expect(commit.error).toBeNull();
  });

  it("drops a reply that a newer selection has already superseded", async () => {
    const slow = deferred<ReturnType<typeof details>>();
    commands.commitDetails.mockReturnValueOnce(slow.promise);
    commands.commitFiles.mockResolvedValueOnce(files("stale.txt"));

    const first = commit.select(REPO, "old");

    commands.commitDetails.mockResolvedValue(details("new"));
    commands.commitFiles.mockResolvedValue(files("fresh.txt"));
    await commit.select(REPO, "new");

    slow.resolve(details("old"));
    await first;

    expect(commit.oid).toBe("new");
    expect(commit.details?.oid).toBe("new");
    expect(commit.files.map((f) => f.path)).toEqual(["fresh.txt"]);
    expect(commit.loading).toBe(false);
  });

  it("clears everything when the selection goes away", async () => {
    commands.commitDetails.mockResolvedValue(details("abc"));
    commands.commitFiles.mockResolvedValue(files("one.txt"));
    await commit.select(REPO, "abc");

    await commit.select(REPO, null);

    expect(commit.oid).toBeNull();
    expect(commit.details).toBeNull();
    expect(commit.files).toEqual([]);
    expect(commands.commitDetails).toHaveBeenCalledTimes(1);
  });

  it("keeps a backend failure as a structured error", async () => {
    commands.commitDetails.mockResolvedValue({
      status: "error",
      error: { kind: "repoNotFound", data: "id 7" },
    });
    commands.commitFiles.mockResolvedValue(files("one.txt"));

    await commit.select(REPO, "abc");

    expect(commit.error?.detail.kind).toBe("repoNotFound");
    expect(commit.details).toBeNull();
    expect(commit.loading).toBe(false);
  });
});
