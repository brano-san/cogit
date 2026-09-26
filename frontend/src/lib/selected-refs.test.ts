import { describe, expect, it } from "vitest";
import type { RefLabel } from "./format";
import { selectedLabels } from "./selected-refs";

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
