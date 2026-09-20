import { beforeEach, describe, expect, it, vi } from "vitest";

const commands = { diffFile: vi.fn() };

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands }));

const { diff } = await import("./diff.svelte");

const REPO = { 0: 1 } as unknown as import("$lib/ipc").RepoId;
const SPEC = { kind: "workTreeVsIndex" } as const;

function textDiff() {
  return {
    status: "ok" as const,
    data: {
      kind: "text" as const,
      hunks: [],
      eol: { old: "lf" as const, new: "lf" as const, normalized: false },
      lossyEncoding: false,
      language: null,
    },
  };
}

describe("diff store", () => {
  beforeEach(() => {
    commands.diffFile.mockReset();
    commands.diffFile.mockResolvedValue(textDiff());
    diff.clear();
  });

  it("keeps the diff of a file that was not touched", async () => {
    await diff.load(REPO, SPEC, "untouched.txt");

    diff.dropIfAffected(["other.txt"]);

    expect(diff.path).toBe("untouched.txt");
    expect(diff.diff).not.toBeNull();
  });

  it("drops the diff of a file that was just discarded", async () => {
    await diff.load(REPO, SPEC, "gone.txt");

    diff.dropIfAffected(["gone.txt"]);

    expect(diff.path).toBeNull();
    expect(diff.diff).toBeNull();
  });

  it("drops it when the file is one of several touched at once", async () => {
    await diff.load(REPO, SPEC, "b.txt");

    diff.dropIfAffected(["a.txt", "b.txt", "c.txt"]);

    expect(diff.path).toBeNull();
  });

  it("does nothing when no diff is shown", () => {
    expect(() => diff.dropIfAffected(["a.txt"])).not.toThrow();
    expect(diff.path).toBeNull();
  });

  it("does nothing for an empty path list", async () => {
    await diff.load(REPO, SPEC, "kept.txt");

    diff.dropIfAffected([]);

    expect(diff.path).toBe("kept.txt");
  });
});
