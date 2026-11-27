import { describe, expect, it } from "vitest";
import type { ScanHit } from "$lib/ipc";
import { scanFilter, scanRange, scanRows, scanSections } from "./scan-groups";

const hit = (root: string, extra: Partial<ScanHit> = {}): ScanHit => ({
  root,
  name: root.split(/[\\/]/).pop() ?? root,
  bare: false,
  alreadyOpen: false,
  ...extra,
});

const titles = (sections: ReturnType<typeof scanSections>) =>
  sections.map((section) => [section.title, section.hits.map((each) => each.name)]);

describe("scanSections", () => {
  const hits = [
    hit("C:/src/work/b"),
    hit("C:/src/a"),
    hit("C:/src/old.git", { bare: true }),
    hit("C:/src/work/c", { alreadyOpen: true }),
    hit("C:/src/Apps/d"),
  ];

  it("groups by the full parent path, bare and already open last", () => {
    const sections = scanSections(hits, "folders");
    expect(titles(sections)).toEqual([
      ["C:/src", ["a"]],
      ["C:/src/Apps", ["d"]],
      ["C:/src/work", ["b"]],
      ["C:/src", ["old.git"]],
      ["C:/src/work", ["c"]],
    ]);
    expect(sections.map((section) => section.kind)).toEqual([null, null, null, "bare", "open"]);
    expect(new Set(sections.map((section) => section.id)).size).toBe(sections.length);
  });

  it("draws a single list in discovery order within each kind", () => {
    expect(titles(scanSections(hits, "flat"))).toEqual([
      [null, ["b", "a", "d", "old.git", "c"]],
    ]);
  });

  it("keeps drive roots and identically named directories distinct", () => {
    const sections = scanSections([hit("C:/app"), hit("D:/app"), hit("/app")], "folders");
    expect(sections.map((section) => section.title)).toEqual(["/", "C:/", "D:/"]);
  });

  it("opens matching groups during search and restores explicit and default expansion", () => {
    const sections = scanSections([
      ...Array.from({ length: 12 }, (_, at) => hit(`/large/repo-${at}`)),
      hit("/small/repo"),
      hit("/other/example"),
    ], "folders");
    const small = sections.find((section) => section.title === "/small")!;
    const expanded = new Map([[small.id, false]]);
    const before = scanRows(sections, expanded, false);
    expect(before.map((row) => row.kind)).toEqual(["group", "group", "repo", "group"]);
    const filtering = scanFilter("repo", "ignore");
    const shown = sections
      .map((section) => ({ ...section, hits: section.hits.filter(filtering.matches) }))
      .filter((section) => section.hits.length > 0);
    expect(scanRows(shown, new Map(), true).filter((row) => row.kind === "repo")).toHaveLength(13);
    expect(scanRows(sections, expanded, false)).toEqual(before);
  });

  it("takes a range in visible row order and skips headers, collapsed and already open items", () => {
    const first = hit("/a/start");
    const middle = hit("/b/middle");
    const hidden = hit("/collapsed/hidden");
    const unavailable = hit("/b/loaded", { alreadyOpen: true });
    const last = hit("/z/end");
    const sections = scanSections([first, middle, hidden, unavailable, last], "flat");
    const flat = scanRows(sections, new Map(), false);
    expect(scanRange(flat, last, first)).toEqual([first.root, middle.root, hidden.root, last.root]);
    const grouped = scanSections([first, middle, hidden, last], "folders");
    const group = grouped.find((section) => section.title === "/collapsed")!;
    const rows = scanRows(grouped, new Map([[group.id, false]]), false);
    expect(scanRange(rows, hidden, middle)).toEqual([middle.root]);
    expect(scanRange(rows, last, first)).toEqual([first.root, middle.root, last.root]);
    expect(scanRange(rows, { ...middle }, first)).toEqual([first.root]);
  });
});

describe("scanFilter", () => {
  const repo = hit("C:/Work/a+b/app", { name: "Alternate" });

  it("matches names, native Windows paths, and literal repeated substrings", () => {
    expect(scanFilter(" alternate ", "smart").matches(repo)).toBe(true);
    expect(scanFilter("work\\a+b", "smart").matches(repo)).toBe(true);
    const search = scanFilter("a+b", "ignore");
    expect(search.matches(repo)).toBe(true);
    expect(search.parts("A+B and a+b")).toEqual([
      { text: "A+B", match: true },
      { text: " and ", match: false },
      { text: "a+b", match: true },
    ]);
  });

  it("highlights the visible pieces of a match that crosses the parent path and name", () => {
    const search = scanFilter("Work/a+b/app", "match");
    const root = "C:/Work/a+b/app";
    expect(search.parts("C:/Work/a+b", true, root)).toEqual([
      { text: "C:/", match: false },
      { text: "Work/a+b", match: true },
    ]);
    expect(search.parts("app", false, root)).toEqual([{ text: "app", match: true }]);
  });
  it("respects the configured case sensitivity in both matching and highlighting", () => {
    const search = scanFilter("ALTERNATE", "smart");
    expect(search.matches(repo)).toBe(false);
    expect(search.parts(repo.name)).toEqual([{ text: "Alternate", match: false }]);
  });
});
