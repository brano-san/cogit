import { expect, it } from "vitest";
import { splitUpstream } from "./push-to";
import { refPicks } from "./ref-selection";
import { leavesUnder, type RefNode } from "./ref-nodes";
import { editorSide } from "./file-dialogs";
import { conflictCount, autoResolvedCount, syntacticCount } from "./merge-view";
import { currentRemote, pullSteps, remotesInOrder } from "./toolbar-prefs";
import { localNameOf } from "./ref-menus";
import type { Region } from "./ipc";

// Previous implementations are kept here as equivalence and timing references.
const oldSplit = (upstream: string, remotes: readonly string[]) => {
  const owner = [...remotes].sort((a, b) => b.length - a.length).find((r) => upstream.startsWith(`${r}/`));
  return owner ? { remote: owner, branch: upstream.slice(owner.length + 1) } : null;
};
const oldPicks = (nodes: readonly RefNode[]) => ({
  locals: nodes.filter((n) => n.kind === "local" && n.branch !== undefined),
  remotes: nodes.filter((n) => n.kind === "remote" && n.branch !== undefined && n.label !== "HEAD"),
  tags: nodes.filter((n) => n.kind === "tag" && n.tag !== undefined),
});
const oldLeaves = (nodes: readonly RefNode[], id: string) => {
  const at = nodes.findIndex((n) => n.id === id);
  if (at < 0) return [];
  const node = nodes[at]!;
  if (node.kind !== "group" && node.kind !== "folder") return node.disabled ? [] : [id];
  const result: string[] = [];
  for (const next of nodes.slice(at + 1)) {
    if (next.depth <= node.depth) break;
    if (next.rev !== undefined && !next.disabled) result.push(next.id);
  }
  return result;
};
const oldEditor = (raw: string | null) => raw === null ? { text: "", endings: [], present: false } : {
  text: raw.replaceAll("\r\n", "\n"), endings: [...raw.matchAll(/\r?\n/g)].map((m) => m[0]), present: true,
};
const oldCount = (regions: readonly Region[]) => regions.filter((r) => r.kind === "conflict").length;

const nodes: RefNode[] = Array.from({ length: 10_000 }, (_, i) => ({
  id: String(i), label: i % 11 === 0 ? "HEAD" : String(i), depth: i % 20 === 0 ? 0 : 1,
  kind: i % 20 === 0 ? "group" : i % 3 === 0 ? "local" : i % 3 === 1 ? "remote" : "tag",
  rev: String(i), disabled: i % 17 === 0 ? "disabled" : undefined,
  branch: i % 7 === 0 ? undefined : { name: String(i) } as never,
  tag: i % 7 === 0 ? undefined : { name: String(i) } as never,
}));
const remotes = Array.from({ length: 1000 }, (_, i) => `team/${i}`);
const raw = "one\r\ntwo\nthree\rfour\r\n".repeat(10_000);
const regions = Array.from({ length: 10_000 }, (_, i) => ({
  kind: i % 3 === 0 ? "conflict" : "clean", origin: i % 2 === 0 ? "syntactic" : "unchanged",
})) as Region[];

it("preserves selection order, subtree boundaries, remote prefixes and line endings", () => {
  expect(refPicks(Object.freeze(nodes))).toEqual(oldPicks(nodes));
  for (const id of ["0", "1", "17", "20", "9999", "absent"]) {
    expect(leavesUnder(nodes, id)).toEqual(oldLeaves(nodes, id));
  }
  for (const names of [[], ["", "team", "team/mirror", "origin"], remotes]) {
    for (const upstream of ["", "/main", "team/mirror/main", "team/999/main", "gone/main"]) {
      expect(splitUpstream(upstream, Object.freeze(names))).toEqual(oldSplit(upstream, names));
    }
  }
  for (const text of [null, "", "\r", "\n", "\r\r\n", "a\r\nb\nc", raw]) {
    expect(editorSide(text)).toEqual(oldEditor(text));
  }
  expect(conflictCount(regions)).toBe(oldCount(regions));
  expect(conflictCount([])).toBe(0);
  expect(autoResolvedCount(regions)).toBe(regions.filter((r) => r.kind === "clean" && r.origin !== "unchanged").length);
  expect(syntacticCount(regions)).toBe(regions.filter((r) => r.kind === "clean" && r.origin === "syntactic").length);
  for (const names of [[], ["", "team", "team/mirror", "origin", "origin"], remotes]) {
    for (const upstream of ["", "/main", "team/mirror/main", "team/999/main", "gone/main", "main"]) {
      const owner = oldSplit(upstream, names);
      expect(currentRemote(upstream, names)).toBe(owner?.remote ?? (names.includes("origin") ? "origin" : names[0] ?? null));
      expect(localNameOf(upstream, names)).toBe(owner?.branch ?? upstream.slice(upstream.indexOf("/") + 1));
      expect(pullSteps("all", names, upstream)).toEqual({ fetch: remotesInOrder(names, upstream).filter((r) => r !== upstream), pull: upstream });
    }
  }
});

it.skipIf(!process.env.COGIT_ALLOCATION_BENCH)("reports timings without machine-dependent thresholds", () => {
  const cases: [string, () => unknown, () => unknown][] = [
    ["remote", () => oldSplit("team/999/main", remotes), () => splitUpstream("team/999/main", remotes)],
    ["selection", () => oldPicks(nodes), () => refPicks(nodes)],
    ["subtree", () => oldLeaves(nodes, "0"), () => leavesUnder(nodes, "0")],
    ["editor", () => oldEditor(raw), () => editorSide(raw)],
    ["conflicts", () => oldCount(regions), () => conflictCount(regions)],
    ["auto-resolved", () => regions.filter((r) => r.kind === "clean" && r.origin !== "unchanged").length, () => autoResolvedCount(regions)],
    ["syntactic", () => regions.filter((r) => r.kind === "clean" && r.origin === "syntactic").length, () => syntacticCount(regions)],
    ["current-remote", () => remotes.filter((r) => "team/999/main".startsWith(`${r}/`)).sort((a, b) => b.length - a.length)[0], () => currentRemote("team/999/main", remotes)],
    ["local-name", () => oldSplit("team/999/main", remotes)?.branch, () => localNameOf("team/999/main", remotes)],
    ["pull-steps", () => remotesInOrder(remotes, "team/999").filter((r) => r !== "team/999"), () => pullSteps("all", remotes, "team/999")],
  ];
  const map = new Map(remotes.map((r, i) => [r, i]));
  const set = new Set(remotes);
  const without = () => { const next = new Set(set); next.delete("team/999"); return next; };
  expect(new Map(map).set("team/999", 42)).toEqual(new Map([...map, ["team/999", 42]]));
  expect(without()).toEqual(new Set([...set].filter((r) => r !== "team/999")));
  cases.push(
    ["map-update", () => new Map([...map, ["team/999", 42]]), () => new Map(map).set("team/999", 42)],
    ["set-add", () => new Set([...set, "another"]), () => new Set(set).add("another")],
    ["set-delete", () => new Set([...set].filter((r) => r !== "team/999")), without],
  );
  const measure = (run: () => unknown) => {
    for (let i = 0; i < 20; i++) run();
    const start = performance.now();
    for (let i = 0; i < 100; i++) run();
    return (performance.now() - start) / 100;
  };
  for (const [name, before, after] of cases) {
    console.log(`${name}: before=${measure(before).toFixed(4)} after=${measure(after).toFixed(4)} ms`);
  }
});
