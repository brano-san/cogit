import { describe, expect, it, vi } from "vitest";

const commands = { openBlameWindow: vi.fn() };
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands }));

const {
  age,
  changedSince,
  clampLine,
  cursorAfter,
  openBlame,
  parseBlame,
  sinceOptions,
  startsBlock,
  viewOptions,
} = await import("./blame-window");

const row = (oid: string) => ({ oid });

describe("parseBlame", () => {
  it("reads what the backend put in the URL", () => {
    expect(parseBlame("?repo=3&path=a%20b/c%26d%23e%2Bf%25%C3%A9.rs&rev=abc")).toEqual({
      repo: 3,
      path: "a b/c&d#e+f%é.rs",
      rev: "abc",
    });
  });

  it("refuses a URL missing any of the three", () => {
    expect(parseBlame("?repo=3&path=a.txt")).toBeNull();
    expect(parseBlame("?path=a.txt&rev=abc")).toBeNull();
    expect(parseBlame("?repo=x&path=a.txt&rev=abc")).toBeNull();
  });
});

describe("changedSince", () => {
  const revisions = [row("c4"), row("c3"), row("c2"), row("c1")];

  it("marks the commits above the chosen one in the newest-first list", () => {
    expect([...changedSince(revisions, "c2")].sort()).toEqual(["c3", "c4"]);
  });

  it("marks nothing without a choice", () => {
    expect(changedSince(revisions, null).size).toBe(0);
  });

  it("marks nothing since the newest commit", () => {
    expect(changedSince(revisions, "c4").size).toBe(0);
  });

  it("marks nothing for a commit that is not a version of the file", () => {
    expect(changedSince(revisions, "elsewhere").size).toBe(0);
  });
});

describe("startsBlock", () => {
  const lines = [row("a"), row("a"), row("b"), row("a")];

  it("annotates the first line of each run, as git blame does", () => {
    expect(lines.map((_, at) => startsBlock(lines, at))).toEqual([true, false, true, true]);
  });
});

describe("age", () => {
  const now = 1_000_000_000;

  it("says minutes, hours, days and years the short way", () => {
    expect(age(now - 30, now)).toBe("now");
    expect(age(now - 45 * 60, now)).toBe("45m");
    expect(age(now - 5 * 3600, now)).toBe("5h");
    expect(age(now - 140 * 86_400, now)).toBe("140d");
    expect(age(now - 3 * 365 * 86_400, now)).toBe("3y");
  });

  it("does not go negative for a clock that is behind the commit", () => {
    expect(age(now + 100, now)).toBe("now");
  });
});

describe("cursorAfter", () => {
  it("moves by one, by a page, and to either end", () => {
    expect(cursorAfter("ArrowDown", 4, 10, 3)).toBe(5);
    expect(cursorAfter("ArrowUp", 4, 10, 3)).toBe(3);
    expect(cursorAfter("PageDown", 4, 10, 3)).toBe(7);
    expect(cursorAfter("PageUp", 4, 10, 3)).toBe(1);
    expect(cursorAfter("Home", 4, 10, 3)).toBe(0);
    expect(cursorAfter("End", 4, 10, 3)).toBe(9);
  });

  it("stays inside the file", () => {
    expect(cursorAfter("ArrowUp", 0, 10, 3)).toBe(0);
    expect(cursorAfter("PageDown", 8, 10, 3)).toBe(9);
  });

  it("has nothing to say about other keys or an empty file", () => {
    expect(cursorAfter("Enter", 4, 10, 3)).toBeNull();
    expect(cursorAfter("ArrowDown", 0, 0, 3)).toBeNull();
  });
});

describe("clampLine", () => {
  it("keeps a line number from one version inside the next", () => {
    expect(clampLine(12, 30)).toBe(11);
    expect(clampLine(40, 30)).toBe(29);
    expect(clampLine(0, 30)).toBe(0);
    expect(clampLine(5, 0)).toBe(0);
  });
});

describe("the two version lists", () => {
  const commit = (oid: string, summary: string) => ({
    oid,
    parents: [],
    summary,
    authorName: "Ann",
    authorEmail: "ann@x",
    timestamp: 10,
    tzOffsetMinutes: 0,
  });
  const revisions = [commit("b".repeat(40), "second"), commit("a".repeat(40), "first")];
  const date = () => "2 days ago";

  it("reads each version as date, hash, author and subject", () => {
    expect(viewOptions(revisions, revisions[0]!.oid, date)).toEqual([
      ["b".repeat(40), "2 days ago · bbbbbbb · Ann · second"],
      ["a".repeat(40), "2 days ago · aaaaaaa · Ann · first"],
    ]);
  });

  it("keeps the version opened even when that commit did not touch the file", () => {
    const options = viewOptions(revisions, "c".repeat(40), date);
    expect(options[0]).toEqual(["c".repeat(40), "ccccccc · the version opened"]);
    expect(options).toHaveLength(3);
  });

  it("offers highlighting nothing first", () => {
    const options = sinceOptions(revisions, date);
    expect(options[0]).toEqual(["", "Nothing"]);
    expect(options).toHaveLength(3);
  });
});

describe("openBlame", () => {
  it("asks the backend for the window, so every entry point opens the same one", async () => {
    commands.openBlameWindow.mockResolvedValue({ status: "ok", data: null });
    await openBlame(2, "src/a.rs", "HEAD");
    expect(commands.openBlameWindow).toHaveBeenCalledWith(2, "src/a.rs", "HEAD");
  });

  it("throws what the backend said", async () => {
    commands.openBlameWindow.mockResolvedValue({
      status: "error",
      error: { kind: "invalidState", data: "cannot resolve nope" },
    });
    await expect(openBlame(2, "src/a.rs", "nope")).rejects.toThrow();
  });
});
