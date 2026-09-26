import { describe, expect, it, vi, beforeEach } from "vitest";

const calls = vi.hoisted(() => ({
  text: new Map<string, (v: unknown) => void>(),
  preview: new Map<string, (v: unknown) => void>(),
}));

vi.mock("$lib/ipc", () => ({
  conflictedPaths: vi.fn(async () => []),
  conflictText: vi.fn(
    (_repo: unknown, path: string) =>
      new Promise((resolve) => calls.text.set(path, resolve as (v: unknown) => void)),
  ),
  mergePreview: vi.fn(
    (_repo: unknown, path: string) =>
      new Promise((resolve) => calls.preview.set(path, resolve as (v: unknown) => void)),
  ),
  resolveConflict: vi.fn(async () => {}),
  resolveConflictText: vi.fn(async () => {}),
}));

const report = vi.hoisted(() => vi.fn());
vi.mock("$stores/notices.svelte", () => ({ notices: { report } }));

const ipc = await import("$lib/ipc");
const { conflicts } = await import("./conflicts.svelte");

const sides = (name: string) => ({ base: name, ours: name, theirs: name, binary: false });
const region = (name: string) => [{ kind: "clean", lines: [name], origin: "ours" }];

beforeEach(() => {
  calls.text.clear();
  calls.preview.clear();
  conflicts.clear();
});

describe("opening one conflicted file after another", () => {
  it("never shows one file's regions under another file's name", async () => {
    const first = conflicts.open(1 as never, "a.txt");
    calls.text.get("a.txt")?.(sides("a.txt"));
    await Promise.resolve();

    const second = conflicts.open(1 as never, "b.txt");
    calls.text.get("b.txt")?.(sides("b.txt"));
    await Promise.resolve();

    // The slower first request answers last; it must not land on the second file.
    calls.preview.get("b.txt")?.(region("b.txt"));
    calls.preview.get("a.txt")?.(region("a.txt"));
    await Promise.all([first, second]);

    expect(conflicts.path).toBe("b.txt");
    expect(conflicts.regions).toEqual(region("b.txt"));
  });

  it("does not reopen a panel the user closed while it was loading", async () => {
    const opening = conflicts.open(1 as never, "a.txt");
    calls.text.get("a.txt")?.(sides("a.txt"));
    await Promise.resolve();

    conflicts.close();
    calls.preview.get("a.txt")?.(region("a.txt"));
    await opening;

    expect(conflicts.path).toBeNull();
    expect(conflicts.regions).toEqual([]);
  });

  it("does not leave the sides of an abandoned file behind", async () => {
    const first = conflicts.open(1 as never, "a.txt");
    const second = conflicts.open(1 as never, "b.txt");
    calls.text.get("b.txt")?.(sides("b.txt"));
    await Promise.resolve();
    calls.preview.get("b.txt")?.(region("b.txt"));
    calls.text.get("a.txt")?.(sides("a.txt"));
    calls.preview.get("a.txt")?.(region("a.txt"));
    await Promise.all([first, second]);

    expect(conflicts.ours).toBe("b.txt");
  });
});

// Taking a side closed the view once Git answered, whichever file was open by then: open
// b while a's resolution was on its way and b's view went too.
describe("resolving a file and opening the next before Git answers", () => {
  it("leaves the next file open", async () => {
    const ipc = await import("$lib/ipc");
    vi.mocked(ipc.conflictedPaths).mockResolvedValue(["b.txt"]);
    let resolved!: () => void;
    vi.mocked(ipc.resolveConflict).mockReturnValueOnce(new Promise((resolve) => (resolved = () => resolve(null as never))));

    const openA = conflicts.open(1 as never, "a.txt");
    calls.text.get("a.txt")?.(sides("a.txt"));
    await vi.waitFor(() => expect(calls.preview.has("a.txt")).toBe(true));
    calls.preview.get("a.txt")?.(region("a.txt"));
    await openA;
    const taking = conflicts.take(1 as never, "ours");
    const openB = conflicts.open(1 as never, "b.txt");
    calls.text.get("b.txt")?.(sides("b.txt"));
    await vi.waitFor(() => expect(calls.preview.has("b.txt")).toBe(true));
    calls.preview.get("b.txt")?.(region("b.txt"));
    await openB;

    resolved();
    await taking;

    expect(conflicts.path).toBe("b.txt");
    expect(conflicts.ours).toBe("b.txt");
  });
});

describe("the list after a mutation", () => {
  it("takes the paths a status read already brought, asking nothing", async () => {
    const { conflictedPaths } = await import("$lib/ipc");
    vi.mocked(conflictedPaths).mockClear();

    await conflicts.refresh(1 as never, ["b.txt", "c.txt"]);

    expect(conflictedPaths).not.toHaveBeenCalled();
    expect(conflicts.paths).toEqual(["b.txt", "c.txt"]);
  });

  it("still asks when nobody brought the list", async () => {
    const { conflictedPaths } = await import("$lib/ipc");
    vi.mocked(conflictedPaths).mockClear();

    await conflicts.refresh(1 as never);

    expect(conflictedPaths).toHaveBeenCalledTimes(1);
  });
});

describe("a conflict that is not text", () => {
  it("is marked binary and never offered to the merge view", async () => {
    const ipc = await import("$lib/ipc");
    vi.mocked(ipc.mergePreview).mockClear();
    const opening = conflicts.open(1 as never, "pic.png");
    calls.text.get("pic.png")?.({ ...sides("pic.png"), binary: true });
    await opening;

    expect(conflicts.binary).toBe(true);
    expect(conflicts.regions).toEqual([]);
    expect(ipc.mergePreview).not.toHaveBeenCalled();
  });

  it("forgets the mark once the panel closes", async () => {
    const opening = conflicts.open(1 as never, "pic.png");
    calls.text.get("pic.png")?.({ ...sides("pic.png"), binary: true });
    await opening;

    conflicts.close();

    expect(conflicts.binary).toBe(false);
  });
});

async function opened(path: string): Promise<void> {
  const opening = conflicts.open(1 as never, path);
  calls.text.get(path)?.(sides(path));
  await vi.waitFor(() => expect(calls.preview.has(path)).toBe(true));
  calls.preview.get(path)?.(region(path));
  await opening;
}

// a popped out to its window, b open in the main one with sides picked: Save in a's
// window closed b's merge view, and the picks went with it.
describe("a file resolved in the merge window", () => {
  it("closes its own view in the main window", async () => {
    await opened("a.txt");

    conflicts.resolvedElsewhere("a.txt");

    expect(conflicts.path).toBeNull();
  });

  it("leaves another file's view open", async () => {
    await opened("b.txt");

    conflicts.resolvedElsewhere("a.txt");

    expect(conflicts.path).toBe("b.txt");
    expect(conflicts.regions).toEqual(region("b.txt"));
  });
});

// A second click on the open conflicted file — the second click of a double-click too —
// opened it again: the merge view was rebuilt and every side picked was gone.
describe("clicking the conflicted file that is open", () => {
  it("keeps the merge on screen as it is", async () => {
    const ipc = await import("$lib/ipc");
    await opened("a.txt");
    vi.mocked(ipc.conflictText).mockClear();

    await conflicts.open(1 as never, "a.txt");

    expect(ipc.conflictText).not.toHaveBeenCalled();
    expect(conflicts.regions).toEqual(region("a.txt"));
  });
});

// Opening any other file — another conflict, a staged file, a commit's file — replaced
// the merge without a word, or never replaced it at all.
describe("leaving an open merge", () => {
  it("closes it at once when nothing was picked", async () => {
    const { confirmation } = await import("./confirm.svelte");
    await opened("a.txt");

    expect(await conflicts.leave()).toBe(true);

    expect(conflicts.path).toBeNull();
    expect(confirmation.open).toBeNull();
  });

  it("asks first when sides were picked, and stays when told to", async () => {
    const { confirmation } = await import("./confirm.svelte");
    await opened("a.txt");
    conflicts.markUnsaved(true);

    const leaving = conflicts.leave();
    expect(confirmation.open?.title).toBe("Discard the Resolution");
    confirmation.answer(false);

    expect(await leaving).toBe(false);
    expect(conflicts.path).toBe("a.txt");
  });

  it("goes once the user agrees", async () => {
    const { confirmation } = await import("./confirm.svelte");
    await opened("a.txt");
    conflicts.markUnsaved(true);

    const leaving = conflicts.leave();
    confirmation.answer(true);

    expect(await leaving).toBe(true);
    expect(conflicts.path).toBeNull();
    expect(conflicts.unsaved).toBe(false);
  });

  it("is nothing to ask about when no merge is open", async () => {
    expect(await conflicts.leave()).toBe(true);
  });
});

// Staged files, a commit's files, a stash's and a comparison's all loaded their diff
// under the merge, which was checked first and stayed on screen until Cancel.
describe("another commit picked in the graph", () => {
  it("closes a merge with nothing picked, and keeps one with picks", async () => {
    await opened("a.txt");
    conflicts.markUnsaved(true);
    conflicts.closeUnlessUnsaved();
    expect(conflicts.path).toBe("a.txt");

    conflicts.markUnsaved(false);
    conflicts.closeUnlessUnsaved();
    expect(conflicts.path).toBeNull();
  });
});

// Take ours, Save resolution and opening a conflicted file failed without a word: the
// rejected promise reached nobody, and nothing listens for unhandled rejections (INV-05).
describe("a resolution that fails", () => {
  it("reports a refused take and keeps the file open", async () => {
    await opened("a.txt");
    const refused = new Error("a.txt: Permission denied");
    vi.mocked(ipc.resolveConflict).mockRejectedValueOnce(refused);

    await conflicts.take(1 as never, "ours");

    expect(report).toHaveBeenCalledWith(refused, "Could not resolve the conflict");
    expect(conflicts.path).toBe("a.txt");
  });

  it("reports a refused save", async () => {
    await opened("a.txt");
    const refused = new Error("a.txt is not conflicted");
    vi.mocked(ipc.resolveConflictText).mockRejectedValueOnce(refused);

    await conflicts.write(1 as never, "text");

    expect(report).toHaveBeenCalledWith(refused, "Could not resolve the conflict");
  });

  it("reports a file that cannot be read", async () => {
    const refused = new Error("b.txt is not conflicted");
    vi.mocked(ipc.conflictText).mockRejectedValueOnce(refused);

    await conflicts.open(1 as never, "b.txt");

    expect(report).toHaveBeenCalledWith(refused, "Could not open the conflict");
    expect(conflicts.path).toBeNull();
  });
});
