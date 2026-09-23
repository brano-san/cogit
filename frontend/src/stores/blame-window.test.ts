import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const commands = { blame: vi.fn(), fileRevisions: vi.fn(), lineHistory: vi.fn() };

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands }));

const { blameWindow, HISTORY_DELAY_MS } = await import("./blame-window.svelte");

const ok = <T>(data: T) => ({ status: "ok" as const, data });
const line = (n: number, oid = "c1") => ({
  line: n,
  text: `line ${n}`,
  oid,
  summary: "s",
  author: "A",
  email: "a@x",
  timestamp: 10,
});
const lines = (count: number) => Array.from({ length: count }, (_, i) => line(i + 1));
const REQUEST = { repo: 1, path: "src/a.rs", rev: "c9" };

describe("blame window store", () => {
  beforeEach(() => {
    for (const command of Object.values(commands)) command.mockReset();
    commands.blame.mockResolvedValue(ok(lines(5)));
    commands.fileRevisions.mockResolvedValue(ok([{ oid: "c9" }, { oid: "c1" }]));
    commands.lineHistory.mockResolvedValue(ok([]));
    blameWindow.reset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("opens on the revision in the URL, with its versions and the first line's history", async () => {
    await blameWindow.open(REQUEST);

    expect(commands.blame).toHaveBeenCalledWith(1, "src/a.rs", "c9");
    expect(commands.fileRevisions).toHaveBeenCalledWith(1, "src/a.rs", "c9");
    expect(commands.lineHistory).toHaveBeenCalledWith(1, "src/a.rs", "c9", 1);
    expect(blameWindow.view).toBe("c9");
    expect(blameWindow.lines).toHaveLength(5);
    expect(blameWindow.revisions.map((row) => row.oid)).toEqual(["c9", "c1"]);
  });

  it("keeps the cursor on the same line number in another version, inside the file", async () => {
    await blameWindow.open(REQUEST);
    blameWindow.moveTo(3);
    commands.blame.mockResolvedValue(ok(lines(2)));

    await blameWindow.show("c1");

    expect(blameWindow.view).toBe("c1");
    expect(blameWindow.cursor).toBe(1);
    expect(commands.lineHistory).toHaveBeenLastCalledWith(1, "src/a.rs", "c1", 2);
  });

  it("drops a slow answer for a version the user has already left", async () => {
    await blameWindow.open(REQUEST);
    let answerOld: (value: unknown) => void = () => {};
    commands.blame.mockReturnValueOnce(new Promise((resolve) => (answerOld = resolve)));
    commands.blame.mockResolvedValueOnce(ok(lines(7)));

    const old = blameWindow.show("c1");
    await blameWindow.show("c9");
    answerOld(ok(lines(1)));
    await old;

    expect(blameWindow.view).toBe("c9");
    expect(blameWindow.lines).toHaveLength(7);
  });

  it("waits for the cursor to rest before asking for a line's history", async () => {
    await blameWindow.open(REQUEST);
    vi.useFakeTimers();
    commands.lineHistory.mockClear();

    blameWindow.moveTo(1);
    blameWindow.moveTo(2);
    blameWindow.moveTo(3);
    expect(commands.lineHistory).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(HISTORY_DELAY_MS);
    expect(commands.lineHistory).toHaveBeenCalledTimes(1);
    expect(commands.lineHistory).toHaveBeenCalledWith(1, "src/a.rs", "c9", 4);
  });

  it("says why blame failed instead of showing an empty file as if it were one", async () => {
    commands.blame.mockResolvedValue({
      status: "error",
      error: { kind: "invalidState", data: "cannot blame src/a.rs" },
    });

    await blameWindow.open(REQUEST);

    expect(blameWindow.lines).toHaveLength(0);
    expect(blameWindow.error).toContain("cannot blame src/a.rs");
    expect(commands.lineHistory).not.toHaveBeenCalled();
  });

  it("does not ask for the history while its panel is hidden", async () => {
    await blameWindow.open(REQUEST);
    blameWindow.toggleHistory();
    commands.lineHistory.mockClear();

    await blameWindow.loadHistory();

    expect(commands.lineHistory).not.toHaveBeenCalled();
  });

  it("refreshes the version on screen, not the one it was opened at", async () => {
    await blameWindow.open(REQUEST);
    await blameWindow.show("c1");
    commands.blame.mockClear();

    await blameWindow.refresh();

    expect(commands.blame).toHaveBeenCalledWith(1, "src/a.rs", "c1");
  });
});
