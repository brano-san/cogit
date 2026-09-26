import { describe, expect, it } from "vitest";
import type { RefLabel } from "./format";
import { selectedLabels, withTracked } from "./selected-refs";

const labels: RefLabel[] = [
  { text: "main", kind: "head" },
  { text: "origin=topic", kind: "local", remotes: ["origin"], name: "topic" },
  { text: "other", kind: "local" },
  { text: "origin/feature", kind: "remote" },
  { text: "v1.0", kind: "tag" },
  { text: "stash@{2}", kind: "stash" },
];

const texts = (shown: RefLabel[]) => shown.map((label) => label.text);

describe("selectedLabels", () => {
  it("keeps the labels of ticked refs only", () => {
    const ticked = new Set(["local:other", "tag:v1.0", "stash:2"]);
    expect(texts(selectedLabels(labels, ticked))).toEqual(["other", "v1.0", "stash@{2}"]);
  });

  it("shows HEAD's branch when HEAD or the branch is ticked", () => {
    expect(texts(selectedLabels(labels, new Set(["HEAD"])))).toEqual(["main"]);
    expect(texts(selectedLabels(labels, new Set(["local:main"])))).toEqual(["main"]);
  });

  it("shows a branch and its remote copy on one commit when either is ticked", () => {
    expect(texts(selectedLabels(labels, new Set(["local:topic"])))).toEqual(["origin=topic"]);
    expect(texts(selectedLabels(labels, new Set(["remote:origin/topic"])))).toEqual(["origin=topic"]);
  });

  it("shows a remote branch by its own tick", () => {
    expect(texts(selectedLabels(labels, new Set(["remote:origin/feature"])))).toEqual(["origin/feature"]);
  });

  it("shows nothing when nothing is ticked", () => {
    expect(selectedLabels(labels, new Set())).toEqual([]);
  });
});

describe("withTracked", () => {
  const branches = [
    { name: "main", kind: "local" as const, upstream: "origin/main", isHead: true },
    { name: "topic", kind: "local" as const, upstream: "origin/topic", isHead: false },
    { name: "gone", kind: "local" as const, upstream: "origin/gone", isHead: false },
    { name: "solo", kind: "local" as const, upstream: null, isHead: false },
    { name: "origin/main", kind: "remote" as const, upstream: null, isHead: false },
    { name: "origin/topic", kind: "remote" as const, upstream: null, isHead: false },
  ];

  it("adds the remote branch a ticked branch tracks", () => {
    expect([...withTracked(new Set(["local:topic"]), branches)]).toEqual(["local:topic", "remote:origin/topic"]);
  });

  it("adds HEAD's upstream while HEAD is ticked", () => {
    expect([...withTracked(new Set(["HEAD"]), branches)]).toEqual(["HEAD", "remote:origin/main"]);
  });

  it("adds nothing for a branch without upstream or with one no longer fetched", () => {
    const ticked = new Set(["local:solo", "local:gone"]);
    expect(withTracked(ticked, branches)).toBe(ticked);
  });
});
