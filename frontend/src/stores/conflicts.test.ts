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
