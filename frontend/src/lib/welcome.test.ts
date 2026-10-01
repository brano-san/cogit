import { describe, expect, it } from "vitest";
import {
  availabilityOf,
  checkAvailability,
  clampSelection,
  defaultSelection,
  folderPlan,
  moveSelection,
  mruRows,
  noteFor,
  okAction,
  repoName,
  selectOption,
  shouldShowAtStartup,
  type Availability,
  type WelcomeSelection,
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

describe("defaultSelection", () => {
  it("is option 3 on the first row when there is a list", () => {
    expect(defaultSelection(3)).toEqual({ option: 3, row: 0 });
  });

  it("is option 1 when the list is empty", () => {
    expect(defaultSelection(0)).toEqual({ option: 1, row: null });
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

describe("keyboard", () => {
  it("walks the options and then the rows", () => {
    let sel: WelcomeSelection = { option: 1, row: null };
    sel = moveSelection(sel, "down", 3);
    expect(sel.option).toBe(2);
    sel = moveSelection(sel, "down", 3);
    expect(sel).toEqual({ option: 3, row: 0 });
    sel = moveSelection(sel, "down", 3);
    expect(sel).toEqual({ option: 3, row: 1 });
    sel = moveSelection(sel, "down", 3);
    sel = moveSelection(sel, "down", 3);
    expect(sel).toEqual({ option: 3, row: 2 });
  });

  it("walks back from the first row to option 2, and stops at option 1", () => {
    expect(moveSelection({ option: 3, row: 0 }, "up", 3).option).toBe(2);
    expect(moveSelection({ option: 1, row: null }, "up", 3).option).toBe(1);
  });

  it("with no rows, option 3 is a stop of its own", () => {
    expect(moveSelection({ option: 2, row: null }, "down", 0)).toEqual({ option: 3, row: null });
    expect(moveSelection({ option: 3, row: null }, "down", 0)).toEqual({ option: 3, row: null });
  });

  it("choosing option 3 lands on the row it left, or the first", () => {
    expect(selectOption({ option: 1, row: 2 }, 3, 4)).toEqual({ option: 3, row: 2 });
    expect(selectOption({ option: 1, row: null }, 3, 4)).toEqual({ option: 3, row: 0 });
    expect(selectOption({ option: 3, row: 1 }, 1, 4).option).toBe(1);
  });

  it("keeps the selection inside a list that shrank", () => {
    expect(clampSelection({ option: 3, row: 4 }, 3)).toEqual({ option: 3, row: 2 });
    expect(clampSelection({ option: 3, row: 0 }, 0)).toEqual({ option: 1, row: null });
    expect(clampSelection({ option: 2, row: 5 }, 2)).toEqual({ option: 2, row: 1 });
  });
});

describe("okAction", () => {
  const rows = mruRows(["D:\\a", "D:\\b"]);

  it("runs the selected option", () => {
    expect(okAction({ option: 1, row: null }, rows)).toEqual({ kind: "folder" });
    expect(okAction({ option: 2, row: null }, rows)).toEqual({ kind: "clone" });
    expect(okAction({ option: 3, row: 1 }, rows)).toEqual({ kind: "open", path: "D:\\b" });
  });

  it("has nothing to do for option 3 without a row", () => {
    expect(okAction({ option: 3, row: null }, [])).toBeNull();
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
