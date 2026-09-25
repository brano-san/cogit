import { beforeEach, describe, expect, it, vi } from "vitest";

const commands = { diffFile: vi.fn(), imageSides: vi.fn(), discardSelection: vi.fn(), stageSelection: vi.fn() };

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
      oldTotal: 0,
      newTotal: 0,
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

  it("knows it already shows a file, so a second click is not a toggle (#7)", async () => {
    await diff.load(REPO, SPEC, "a.txt");

    expect(diff.shows(SPEC, "a.txt")).toBe(true);
    expect(diff.shows({ kind: "indexVsHead" }, "a.txt")).toBe(false);
    expect(diff.shows(SPEC, "b.txt")).toBe(false);
  });

  it("tells commits apart by their id", async () => {
    await diff.load(REPO, { kind: "commitVsParent", oid: "c1" }, "a.txt");

    expect(diff.shows({ kind: "commitVsParent", oid: "c1" }, "a.txt")).toBe(true);
    expect(diff.shows({ kind: "commitVsParent", oid: "c2" }, "a.txt")).toBe(false);
  });

  it("lets a failed diff be asked for again", async () => {
    commands.diffFile.mockResolvedValue({ status: "error", error: { kind: "internal", data: "x" } });
    await diff.load(REPO, SPEC, "a.txt");

    expect(diff.shows(SPEC, "a.txt")).toBe(false);
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

describe("clicking from one image to another", () => {
  beforeEach(() => diff.clear());

  // The pictures take a second round trip; the first file's used to land under the second.
  it("never shows the first image's pictures under the second", async () => {
    commands.diffFile.mockResolvedValue({ status: "ok", data: { kind: "image" } });
    let first!: (value: unknown) => void;
    commands.imageSides.mockReturnValueOnce(new Promise((resolve) => (first = resolve)));
    commands.imageSides.mockResolvedValueOnce({ status: "ok", data: ["b-old", "b-new"] });

    const loadingA = diff.load(REPO, SPEC, "a.png");
    await vi.waitFor(() => expect(commands.imageSides).toHaveBeenCalledOnce());
    await diff.load(REPO, SPEC, "b.png");
    first({ status: "ok", data: ["a-old", "a-new"] });
    await loadingA;

    expect(diff.path).toBe("b.png");
    expect(diff.images).toEqual(["b-old", "b-new"]);
  });
});

// Discard names the lines of the diff on screen. The store used to send them with the path
// of whatever was asked for last: click b while a is still shown, confirm a's Discard, and
// a's line numbers were thrown away in b.
describe("discarding lines while the diff changes", () => {
  beforeEach(() => {
    diff.clear();
    commands.diffFile.mockReset();
    commands.discardSelection.mockReset();
    commands.discardSelection.mockResolvedValue({ status: "ok", data: null });
    commands.diffFile.mockImplementation(async () => textDiff());
  });

  it("throws away the lines in the file they were chosen in", async () => {
    await diff.load(REPO, SPEC, "a.txt");
    const chosen = diff.diff!;
    commands.diffFile.mockReturnValueOnce(new Promise(() => {}));
    void diff.load(REPO, SPEC, "b.txt");

    await diff.discardLines(new Set(["d:1"]), chosen);

    expect(commands.discardSelection).toHaveBeenCalledOnce();
    expect(commands.discardSelection.mock.calls[0]?.[1].path).toBe("a.txt");
  });

  it("does nothing once another diff has replaced the one they were chosen in", async () => {
    await diff.load(REPO, SPEC, "a.txt");
    const chosen = diff.diff!;
    await diff.load(REPO, SPEC, "b.txt");

    await diff.discardLines(new Set(["d:1"]), chosen);

    expect(commands.discardSelection).not.toHaveBeenCalled();
  });
});

// Stage, Unstage and Discard were offered alike on both sides of the index. On the
// staged diff, Discard reversed the index-against-HEAD patch in the working tree: the
// working tree went back to HEAD and the change stayed staged, ready to be committed.
describe("line actions and the side of the index a diff shows", () => {
  beforeEach(() => {
    diff.clear();
    commands.diffFile.mockReset();
    commands.discardSelection.mockReset();
    commands.discardSelection.mockResolvedValue({ status: "ok", data: null });
    commands.diffFile.mockImplementation(async () => textDiff());
  });

  it("offers Stage and Discard on unstaged changes, Unstage on staged ones", async () => {
    await diff.load(REPO, SPEC, "a.txt");
    expect(diff.lineActions).toEqual({ stage: true, unstage: false, discard: true });

    await diff.load(REPO, { kind: "indexVsHead" }, "a.txt");
    expect(diff.lineActions).toEqual({ stage: false, unstage: true, discard: false });
  });

  it("throws nothing away from a staged diff", async () => {
    await diff.load(REPO, { kind: "indexVsHead" }, "a.txt");

    await diff.discardLines(new Set(["d:1"]), diff.diff!);

    expect(commands.discardSelection).not.toHaveBeenCalled();
  });
});

// Stage lines built its request in App from the path asked for last and the hunks on
// screen, the discard's old mix-up: click b while a is shown and a's lines went to b.
describe("staging lines", () => {
  beforeEach(() => {
    diff.clear();
    commands.diffFile.mockReset();
    commands.stageSelection.mockReset();
    commands.stageSelection.mockResolvedValue({ status: "ok", data: null });
    commands.diffFile.mockImplementation(async () => textDiff());
  });

  it("stages them in the file they were chosen in", async () => {
    await diff.load(REPO, SPEC, "a.txt");
    commands.diffFile.mockReturnValueOnce(new Promise(() => {}));
    void diff.load(REPO, SPEC, "b.txt");

    await diff.stageLines(new Set(["i:1"]), false);

    expect(commands.stageSelection.mock.calls[0]?.[1].path).toBe("a.txt");
  });

  it("stages only from unstaged changes and unstages only from staged ones", async () => {
    await diff.load(REPO, { kind: "indexVsHead" }, "a.txt");
    await diff.stageLines(new Set(["i:1"]), false);
    expect(commands.stageSelection).not.toHaveBeenCalled();

    await diff.stageLines(new Set(["i:1"]), true);
    expect(commands.stageSelection).toHaveBeenCalledOnce();
  });
});

// Discard lines went straight to the backend from the view. The file list stayed as it was
// (the watcher is quiet after our own writes) and the journal behind Undo was not read
// again, so Undo said "Nothing to undo" while undo_last would have put the lines back.
describe("discarding lines ends like any other change to the working tree", () => {
  beforeEach(() => {
    diff.clear();
    commands.diffFile.mockReset();
    commands.discardSelection.mockReset();
    commands.discardSelection.mockResolvedValue({ status: "ok", data: null });
    commands.diffFile.mockImplementation(async () => textDiff());
  });

  it("reads the file list and the journal again afterwards", async () => {
    const loadWorktree = vi.fn(async () => {});
    const after = vi.fn(async () => {});
    diff.useMutation({ repo: () => REPO, epoch: () => 0, report: vi.fn(), loadWorktree, after });
    await diff.load(REPO, SPEC, "a.txt");

    await diff.discardLines(new Set(["d:1"]), diff.diff!);
    diff.useMutation(null);

    expect(commands.discardSelection).toHaveBeenCalledOnce();
    expect(loadWorktree).toHaveBeenCalledWith(REPO);
    expect(after).toHaveBeenCalledWith(["a.txt"]);
  });

  it("reports a refusal where every other failed change goes", async () => {
    const report = vi.fn();
    commands.discardSelection.mockResolvedValue({ status: "error", error: { kind: "invalidState", message: "stale" } });
    diff.useMutation({ repo: () => REPO, epoch: () => 0, report, loadWorktree: vi.fn(), after: vi.fn() });
    await diff.load(REPO, SPEC, "a.txt");

    await diff.discardLines(new Set(["d:1"]), diff.diff!);
    diff.useMutation(null);

    expect(report).toHaveBeenCalledOnce();
  });
});

// The buttons followed the diff asked for last while the lines on screen still belonged to
// the one before: Unstage was live on a working-tree diff and sent its hunks to the index.
describe("line actions while the next diff loads", () => {
  beforeEach(() => {
    diff.clear();
    commands.diffFile.mockReset();
    commands.diffFile.mockImplementation(async () => textDiff());
  });

  it("stay those of the diff on screen", async () => {
    await diff.load(REPO, SPEC, "a.txt");
    commands.diffFile.mockReturnValueOnce(new Promise(() => {}));
    void diff.load(REPO, { kind: "indexVsHead" }, "a.txt");

    expect(diff.lineActions).toEqual({ stage: true, unstage: false, discard: true });
    expect(diff.stageable).toBe(true);
  });
});

// An edit in the editor or `git add` in a terminal left the Unstaged diff on its old
// hunks, and Stage built its patch from them: the old line went into the index.
describe("a change on disk", () => {
  beforeEach(() => {
    commands.diffFile.mockReset();
    commands.diffFile.mockResolvedValue(textDiff());
    diff.clear();
  });

  it("re-reads a diff of the working tree or the index", async () => {
    for (const spec of [SPEC, { kind: "indexVsHead" }, { kind: "commitVsWorkTree", oid: "c1" }] as const) {
      await diff.load(REPO, spec, "a.txt");
      commands.diffFile.mockClear();

      await diff.refreshFromDisk();

      expect(commands.diffFile).toHaveBeenCalledOnce();
      expect(diff.path).toBe("a.txt");
    }
  });

  it("leaves a diff between two commits alone", async () => {
    await diff.load(REPO, { kind: "commitVsParent", oid: "c1" }, "a.txt");
    commands.diffFile.mockClear();

    await diff.refreshFromDisk();

    expect(commands.diffFile).not.toHaveBeenCalled();
  });

  it("clears the panel once the file no longer differs", async () => {
    await diff.load(REPO, SPEC, "a.txt");
    commands.diffFile.mockResolvedValue({ status: "ok", data: { kind: "unchanged" } });

    await diff.refreshFromDisk();

    expect(diff.path).toBeNull();
  });
});
