import { describe, expect, it } from "vitest";
import type { ScanHit } from "$lib/ipc";
import { scanSections } from "./scan-groups";

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
    hit("C:\\src\\work\\b"),
    hit("C:\\src\\a"),
    hit("C:\\src\\old.git", { bare: true }),
    hit("C:\\src\\work\\c", { alreadyOpen: true }),
    hit("C:\\src\\Apps\\d"),
  ];

  it("groups by the folder each one sits in, bare and already open last", () => {
    expect(titles(scanSections(hits, "folders", "C:\\src"))).toEqual([
      ["Apps", ["d"]],
      ["src", ["a"]],
      ["work", ["b"]],
      ["Bare", ["old.git"]],
      ["Already open", ["c"]],
    ]);
  });

  it("keeps one list in the order found, bare and already open still apart", () => {
    expect(titles(scanSections(hits, "flat", "C:/src/"))).toEqual([
      [null, ["b", "a", "d"]],
      ["Bare", ["old.git"]],
      ["Already open", ["c"]],
    ]);
  });
});
