import { beforeEach, describe, expect, it, vi } from "vitest";

const commands = { diffFile: vi.fn() };

/** Stands in for the on-disk store: the branch keeps its view preferences there. */
const stored = new Map<string, unknown>();
let storeFails = false;

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands }));
vi.mock("$lib/settings-file", () => ({
  readKey: async (key: string) => {
    if (storeFails) throw new Error("store unavailable");
    return stored.get(key);
  },
  writeKey: async (key: string, value: unknown) => {
    if (storeFails) throw new Error("store unavailable");
    stored.set(key, value);
  },
}));

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

  it("leaves a file that was not touched alone", async () => {
    await diff.load(REPO, SPEC, "untouched.txt");
    commands.diffFile.mockClear();

    await diff.dropIfAffected(["other.txt"]);

    expect(diff.path).toBe("untouched.txt");
    expect(commands.diffFile).not.toHaveBeenCalled();
  });

  it("re-diffs the shown file instead of blanking the panel", async () => {
    await diff.load(REPO, SPEC, "staged.txt");
    commands.diffFile.mockClear();

    await diff.dropIfAffected(["staged.txt"]);

    expect(diff.path).toBe("staged.txt");
    expect(commands.diffFile).toHaveBeenCalledOnce();
  });

  it("re-diffs it when the file is one of several touched at once", async () => {
    await diff.load(REPO, SPEC, "b.txt");

    await diff.dropIfAffected(["a.txt", "b.txt", "c.txt"]);

    expect(diff.path).toBe("b.txt");
  });

  it("clears the panel once the file no longer differs", async () => {
    await diff.load(REPO, SPEC, "gone.txt");
    commands.diffFile.mockResolvedValue({ status: "ok", data: { kind: "unchanged" } });

    await diff.dropIfAffected(["gone.txt"]);

    expect(diff.path).toBeNull();
    expect(diff.diff).toBeNull();
  });

  it("clears the panel when the file can no longer be diffed at all", async () => {
    await diff.load(REPO, SPEC, "deleted.txt");
    commands.diffFile.mockResolvedValue({
      status: "error",
      error: { kind: "invalidState", data: "no such path" },
    });

    await diff.dropIfAffected(["deleted.txt"]);

    expect(diff.path).toBeNull();
  });

  it("does nothing when no diff is shown", async () => {
    await expect(diff.dropIfAffected(["a.txt"])).resolves.toBeUndefined();
    expect(diff.path).toBeNull();
  });

  it("does nothing for an empty path list", async () => {
    await diff.load(REPO, SPEC, "kept.txt");
    commands.diffFile.mockClear();

    await diff.dropIfAffected([]);

    expect(diff.path).toBe("kept.txt");
    expect(commands.diffFile).not.toHaveBeenCalled();
  });
});

describe("diff view preferences", () => {
  beforeEach(async () => {
    commands.diffFile.mockReset();
    commands.diffFile.mockResolvedValue(textDiff());
    stored.clear();
    storeFails = false;
    diff.clear();
    diff.resetPreferences();
  });

  it("starts side by side, which is what the panel is for", () => {
    expect(diff.layout).toBe("split");
    expect(diff.showMoves).toBe(true);
  });

  it("remembers the layout for the next run", async () => {
    await diff.setLayout("unified");

    expect(stored.get("diffView")).toEqual({ layout: "unified", showMoves: true });
  });

  it("reads the remembered layout back", async () => {
    stored.set("diffView", { layout: "unified", showMoves: false });

    await diff.loadPreferences();

    expect(diff.layout).toBe("unified");
    expect(diff.showMoves).toBe(false);
  });

  it("reads the store only once, however many views ask", async () => {
    stored.set("diffView", { layout: "unified", showMoves: true });

    await diff.loadPreferences();
    stored.set("diffView", { layout: "split", showMoves: true });
    await diff.loadPreferences();

    expect(diff.layout).toBe("unified");
  });

  it("ignores a stored layout that is not a mode it knows", async () => {
    stored.set("diffView", { layout: "three-way", showMoves: true });

    await diff.loadPreferences();

    expect(diff.layout).toBe("split");
  });

  it("ignores stored junk instead of failing to open", async () => {
    stored.set("diffView", "unified");

    await diff.loadPreferences();

    expect(diff.layout).toBe("split");
  });

  it("keeps the default when the store cannot be read", async () => {
    storeFails = true;

    await diff.loadPreferences();

    expect(diff.layout).toBe("split");
  });

  it("still applies the choice when it cannot be written down", async () => {
    storeFails = true;

    await diff.setLayout("unified");

    expect(diff.layout).toBe("unified");
  });

  it("re-runs the diff with move detection off", async () => {
    await diff.load(REPO, SPEC, "moved.rs");
    commands.diffFile.mockClear();

    await diff.setShowMoves(false);

    expect(diff.showMoves).toBe(false);
    const options = commands.diffFile.mock.calls[0]?.[3];
    expect(options.detectMoves).toBe(false);
  });

  it("does not ask the backend again when no file is open", async () => {
    await diff.setShowMoves(false);

    expect(commands.diffFile).not.toHaveBeenCalled();
  });

  it("remembers that moves are shown as ordinary edits", async () => {
    await diff.setShowMoves(false);

    expect(stored.get("diffView")).toEqual({ layout: "split", showMoves: false });
  });
});
