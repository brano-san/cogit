import { describe, expect, it, vi } from "vitest";
import { flushSync } from "svelte";

const commands = { diffFile: vi.fn() };

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands }));
vi.mock("$lib/settings-file", () => ({ readKey: async () => undefined, writeKey: async () => {} }));

const { diff } = await import("./diff.svelte");

const text = (hunks: number) => ({
  status: "ok" as const,
  data: {
    kind: "text" as const,
    hunks: Array.from({ length: hunks }, () => ({ header: "", oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, rows: [] })),
    eol: { old: "lf" as const, new: "lf" as const, normalized: false },
    lossyEncoding: false,
    language: null,
    oldTotal: 1,
    newTotal: 1,
    oldText: null,
    newText: null,
  },
});

describe("the file on screen, as the views read it", () => {
  it("stays put through a re-diff of the same file: a view keeps its scroll and selection", async () => {
    commands.diffFile.mockResolvedValue(text(1));
    await diff.load(1, { kind: "workTreeVsIndex" }, "a.ts");
    let runs = 0;
    const stop = $effect.root(() => {
      $effect(() => {
        void diff.shownPath;
        void diff.shownSpec;
        runs += 1;
      });
    });
    flushSync();

    commands.diffFile.mockResolvedValue(text(2));
    await diff.reload();
    flushSync();
    expect(diff.hunks).toHaveLength(2);
    expect(runs).toBe(1);

    await diff.load(1, { kind: "workTreeVsIndex" }, "b.ts");
    flushSync();
    expect(runs).toBe(2);
    stop();
  });
});
