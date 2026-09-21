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

const sides = (name: string) => ({ base: name, ours: name, theirs: name });
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
