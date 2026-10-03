import { describe, expect, it } from "vitest";
import {
  availabilityOf,
  checkAvailability,
  filterRows,
  folderPlan,
  forgetMissing,
  forgetMissingQuestion,
  keepSelection,
  missingPaths,
  moveSelection,
  mruRows,
  noteFor,
  openTarget,
  repoName,
  selectionAfterRemoval,
  shouldShowAtStartup,
  showsFilter,
  type Availability,
} from "./welcome";

describe("shouldShowAtStartup", () => {
  const base = { enabled: true, restoring: false, openCount: 0, phase: "closed" as const };

  it("shows when nothing is open and the setting is on", () => {
    expect(shouldShowAtStartup(base)).toBe(true);
    expect(shouldShowAtStartup({ ...base, phase: "failed" })).toBe(true);
  });

  it("never shows before the session has settled", () => {
    expect(shouldShowAtStartup({ ...base, restoring: true })).toBe(false);
    expect(shouldShowAtStartup({ ...base, phase: "opening" })).toBe(false);
  });

  it("stays away when a repository is open or the setting is off", () => {
    expect(shouldShowAtStartup({ ...base, openCount: 1, phase: "open" })).toBe(false);
    expect(shouldShowAtStartup({ ...base, enabled: false })).toBe(false);
  });
});

describe("mruRows", () => {
  it("keeps the order of last open and drops repeats, however they are spelled", () => {
    const rows = mruRows(["D:\\work\\app", "D:/work/lib/", "d:\\work\\APP", "D:\\work\\lib"]);
    expect(rows.map((row) => row.path)).toEqual(["D:\\work\\app", "D:/work/lib/"]);
  });

  it("keeps same names apart by their full paths", () => {
    const rows = mruRows(["D:\\a\\app", "D:\\b\\app"]);
    expect(rows.map((row) => row.name)).toEqual(["app", "app"]);
    expect(rows[0]?.path).not.toBe(rows[1]?.path);
  });

  it("does not fold paths that differ only in case on a POSIX path", () => {
    expect(mruRows(["/home/a/App", "/home/a/app"])).toHaveLength(2);
  });

  it("shows a WSL or UNC path in full", () => {
    const wsl = "\\\\wsl.localhost\\Ubuntu\\home\\me\\project";
    const [row] = mruRows([wsl]);
    expect(row).toMatchObject({ path: wsl, name: "project" });
    expect(repoName("\\\\wsl.localhost\\Ubuntu")).toBe("Ubuntu");
  });
});

describe("repoName", () => {
  it("is the last segment, whichever slash and with or without a trailing one", () => {
    expect(repoName("C:\\work\\cogit\\")).toBe("cogit");
    expect(repoName("/home/me/cogit")).toBe("cogit");
  });

  it("falls back to the path for a drive root", () => {
    expect(repoName("C:\\")).toBe("C:\\");
  });
});

describe("availability", () => {
  it("is read from what the folder is", () => {
    expect(availabilityOf("repository")).toBe("available");
    expect(availabilityOf("missing")).toBe("missing");
    expect(availabilityOf("file")).toBe("missing");
    expect(availabilityOf("plain")).toBe("notRepository");
    expect(availabilityOf(null)).toBe("unknown");
  });

  it("says why a row is off, and nothing for the others", () => {
    expect(noteFor("missing")).toBe("not found");
    expect(noteFor("notRepository")).toBe("not a repository");
    expect(noteFor("checking")).toBeNull();
    expect(noteFor("unknown")).toBeNull();
  });

  it("reports each path as its check ends, and a hung one as unknown after the timeout", async () => {
    const seen = new Map<string, Availability>();
    await checkAvailability(
      ["ok", "gone", "hung"],
      (path) => {
        if (path === "hung") return new Promise(() => {});
        return Promise.resolve(path === "ok" ? "repository" : "missing");
      },
      { timeoutMs: 20, onResult: (path, value) => seen.set(path, value) },
    );
    expect(Object.fromEntries(seen)).toEqual({ ok: "available", gone: "missing", hung: "unknown" });
  });

  it("treats a probe that throws as unknown", async () => {
    const seen = new Map<string, Availability>();
    await checkAvailability(["x"], () => Promise.reject(new Error("boom")), {
      timeoutMs: 20,
      onResult: (path, value) => seen.set(path, value),
    });
    expect(seen.get("x")).toBe("unknown");
  });

  it("reports nothing once canceled", async () => {
    const controller = new AbortController();
    const seen: string[] = [];
    const done = checkAvailability(
      ["a", "b"],
      () => new Promise((resolve) => setTimeout(() => resolve("repository"), 10)),
      { timeoutMs: 100, signal: controller.signal, onResult: (path) => seen.push(path) },
    );
    controller.abort();
    await done;
    expect(seen).toEqual([]);
  });

  it("does not wait for the slow ones before reporting the fast ones", async () => {
    const order: string[] = [];
    await checkAvailability(
      ["slow", "fast"],
      (path) => new Promise((resolve) => setTimeout(() => resolve("repository"), path === "slow" ? 30 : 1)),
      { timeoutMs: 200, onResult: (path) => order.push(path) },
    );
    expect(order).toEqual(["fast", "slow"]);
  });
});

describe("filter", () => {
  const rows = mruRows(["D:\\work\\cogit", "D:\\work\\site", "E:\\old\\Cogit-fork"]);

  it("stands over the list only when it is longer than eight rows", () => {
    expect(showsFilter(8)).toBe(false);
    expect(showsFilter(9)).toBe(true);
  });

  it("matches the name or the path, ignoring case", () => {
    expect(filterRows(rows, "COGIT").map((row) => row.name)).toEqual(["cogit", "Cogit-fork"]);
    expect(filterRows(rows, "e:\\old").map((row) => row.name)).toEqual(["Cogit-fork"]);
    expect(filterRows(rows, "  ")).toEqual(rows);
  });

  it("moves the selection to the first row shown when the filter hides it", () => {
    expect(keepSelection(filterRows(rows, "site"), "D:\\work\\cogit")).toBe("D:\\work\\site");
    expect(keepSelection(rows, "D:\\work\\site")).toBe("D:\\work\\site");
    expect(keepSelection([], "D:\\work\\site")).toBeNull();
  });
});

describe("selection", () => {
  const rows = mruRows(["a", "b", "c"]);

  it("walks the rows shown and stops at the ends", () => {
    expect(moveSelection(rows, "a", "down")).toBe("b");
    expect(moveSelection(rows, "c", "down")).toBe("c");
    expect(moveSelection(rows, "a", "up")).toBe("a");
    expect(moveSelection(rows, "b", "end")).toBe("c");
    expect(moveSelection(rows, "c", "home")).toBe("a");
  });

  it("starts at the first row, or the last going up, when nothing is selected", () => {
    expect(moveSelection(rows, null, "down")).toBe("a");
    expect(moveSelection(rows, null, "up")).toBe("c");
    expect(moveSelection([], null, "down")).toBeNull();
  });

  it("after a removal lands on the next row that stays, else the one before", () => {
    expect(selectionAfterRemoval(rows, "b", new Set(["b"]))).toBe("c");
    expect(selectionAfterRemoval(rows, "b", new Set(["b", "c"]))).toBe("a");
    expect(selectionAfterRemoval(rows, "a", new Set(["c"]))).toBe("a");
    expect(selectionAfterRemoval(rows, "a", new Set(["a", "b", "c"]))).toBeNull();
  });
});

describe("missing repositories", () => {
  const rows = mruRows(["ok", "gone", "plain", "hung"]);
  const availability = new Map<string, Availability>([
    ["ok", "available"],
    ["gone", "missing"],
    ["plain", "notRepository"],
    ["hung", "unknown"],
  ]);

  it("Remove All Missing takes the rows not found or no longer a repository", () => {
    expect(missingPaths(rows, availability)).toEqual(["gone", "plain"]);
  });

  it("Remove All Missing asks first, naming how many go", () => {
    expect(forgetMissingQuestion(["a", "b", "c"]).message).toBe("Remove 3 missing repositories from the list?");
    expect(forgetMissingQuestion(["a"]).message).toBe("Remove 1 missing repository from the list?");
  });

  it("Remove All Missing removes only on a yes and asks nothing when none is missing", async () => {
    const forgotten: string[][] = [];
    const forget = (paths: readonly string[]) => void forgotten.push([...paths]);
    let asked = 0;
    const answer = (yes: boolean) => () => {
      asked += 1;
      return Promise.resolve(yes);
    };
    await forgetMissing(["gone"], answer(false), forget);
    expect(forgotten).toEqual([]);
    await forgetMissing(["gone", "plain"], answer(true), forget);
    expect(forgotten).toEqual([["gone", "plain"]]);
    await forgetMissing([], answer(true), forget);
    expect(asked).toBe(2);
  });

  it("Open has nothing to open without a selection or on a missing row", () => {
    expect(openTarget(rows, null, availability)).toBeNull();
    expect(openTarget(rows, "gone", availability)).toBeNull();
    expect(openTarget(rows, "ok", availability)).toBe("ok");
    // A check that ended without an answer stays usable: opening it says what is wrong.
    expect(openTarget(rows, "hung", availability)).toBe("hung");
  });
});

describe("folderPlan", () => {
  it("opens a repository, offers git init for a plain folder, refuses the rest", () => {
    expect(folderPlan("repository")).toBe("open");
    expect(folderPlan("plain")).toBe("init");
    expect(folderPlan("missing")).toBe("gone");
    expect(folderPlan("file")).toBe("gone");
  });
});
