/** Manual scenario timings: COGIT_USAGE_BENCH=before|after npm test -- usage-performance
 * Fixtures represent user actions; the clock includes the whole exported operation.
 * This measures data preparation, not IPC, DOM, paint or disk scanning. */
import { createHash } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { cpus, platform } from "node:os";
import { resolve } from "node:path";
import { expect, it, vi } from "vitest";
import type { Branch, FileEntry, Hunk, ScanHit, Tag } from "$lib/ipc";
import { buildRefTree, type RefTreeInput } from "./ref-nodes";
import { toggleSelected } from "./ref-selection";
import { scanSections } from "./scan-groups";
import { listedRepos } from "./repo-list";
import { sortedFiles } from "./commit-window";
import { commitScope } from "./commit-scope";
import { compile, matches } from "./file-search";
import { sortRows } from "./file-columns";
import { foldDiff, highlightedRows, type FoldEntry } from "./diff-fold";
import { buildBlocks } from "./diff-blocks";
import { alignModel } from "./diff-aligned";

const commands = vi.hoisted(() => ({ scanForRepositories: vi.fn(), cancelOperation: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands }));
const { scan } = await import("$stores/scan.svelte");

it.skipIf(!process.env.COGIT_USAGE_BENCH)("measures complete data operations for fifteen usage scenarios", async () => {
  const files: FileEntry[] = Array.from({ length: 1500 }, (_, i) => ({
    path: `src/package${i % 30}/file${(i * 997) % 1500}.${i % 3 ? "ts" : "svelte"}`,
    oldPath: null, status: i % 5 ? "modified" : "added", mode: "plain", submodule: null,
  }));
  const hits: ScanHit[] = Array.from({ length: 1000 }, (_, i) => ({
    root: `D:/work/team${(i * 137) % 250}/repo${i}`, name: `repo${i}`, bare: false, alreadyOpen: i % 10 === 0,
  }));
  const branches: Branch[] = Array.from({ length: 3000 }, (_, i) => ({
    name: i < 1000 ? `feature/${i}` : `origin/feature/${i - 1000}`,
    fullName: i < 1000 ? `refs/heads/feature/${i}` : `refs/remotes/origin/feature/${i - 1000}`,
    kind: i < 1000 ? "local" : "remote", oid: i.toString(16).padStart(40, "0"), isHead: i === 0,
    upstream: i < 1000 ? `origin/feature/${i}` : null, pushRemote: null, pushTarget: null, ahead: 0, behind: 0,
  }));
  const tags: Tag[] = Array.from({ length: 10000 }, (_, i) => ({
    name: `release/v${i}`, fullName: `refs/tags/release/v${i}`, oid: i.toString(16).padStart(40, "0"),
    isAnnotated: true, pointsToCommit: i % 100 !== 0,
  }));
  const base: RefTreeInput = { head: null, branches: [], tags: [], stashes: [], lost: [], remoteUrls: {}, collapsed: new Set(), filter: "" };
  const selectionTree = buildRefTree({ ...base, tags: tags.slice(0, 4000) });
  const selected = selectionTree.map((node) => node.id);
  const full: Hunk = {
    oldStart: 1, newStart: 1, oldLines: 20000, newLines: 20000, header: "@@ -1,20000 +1,20000 @@",
    rows: Array.from({ length: 20000 }, (_, i) => ({ kind: "context" as const, old: i + 1, new: i + 1, text: `    line ${i}` })),
  };
  // A real changed file with all context loaded, not an Unchanged backend summary.
  full.rows.splice(10000, 1,
    { kind: "delete", old: 10001, text: "    line 10000", inline: [] },
    { kind: "insert", new: 10001, text: "    line 10000 changed", inline: [] },
  );
  const revealed = Array.from({ length: 500 }, (_, i) => ({ from: i * 40 + 1, to: i * 40 + 20 })).reverse();
  const entries: FoldEntry[] = full.rows.map((row) => ({ kind: "row", row, block: 0 }));
  const model = buildBlocks(entries);
  const longLines: Hunk = { ...full, header: "@@ -1,100 +1,100 @@", oldLines: 100, newLines: 100, rows: Array.from({ length: 100 }, (_, i) => ({
    kind: "context", old: i + 1, new: i + 1, text: "const asset = '" + "😀x".repeat(1500) + "';",
  })) };
  longLines.rows.splice(50, 1,
    { kind: "delete", old: 51, text: longLines.rows[50]!.text, inline: [] },
    { kind: "insert", new: 51, text: "const asset = 'updated';", inline: [] },
  );
  // Keep this workload reachable through diff_engine::MAX_TEXT_BYTES, not only in a
  // direct call to the frontend helper.
  expect(Buffer.byteLength(longLines.rows.map((row) => row.text).join("\n"))).toBeLessThan(1_000_000);
  const pattern = compile("file", false);
  const shown = hits.filter((_, i) => i % 2).map((hit) => hit.root);
  const repoList = { closed: hits.slice(0, 200).map((hit) => hit.root), names: {}, pinned: hits.slice(0, 20).map((hit) => hit.root) };

  function hash(value: unknown): string {
    return createHash("sha256").update(JSON.stringify(value, (_key, item) => item instanceof Set || item instanceof Map ? [...item] : item)).digest("hex");
  }

  commands.scanForRepositories.mockImplementation((_path, _depth, channel) => {
    channel.onmessage({ kind: "started", id: 1 });
    for (const hit of hits) channel.onmessage({ kind: "found", hit });
    return Promise.resolve({ status: "ok", data: hits.length });
  });
  const cases: [string, string, () => unknown | Promise<unknown>][] = [
    ["refs.refresh", "1000 local + 2000 remote branches; rebuild Branches", () => buildRefTree({ ...base, branches })],
    ["refs.search", "type five queries over 10000 release tags", () => ["release/v99", "release/v999", "absent", "v1234", "ffff"].map((filter) => buildRefTree({ ...base, tags, filter }))],
    ["scan.groups", "group 1000 discovered repositories into 250 folders", () => scanSections(hits, "folders", "D:/work")],
    ["repos.list", "refresh 200 saved repositories, 20 pinned", () => listedRepos([], repoList)],
    ["commit.sort", "open Commit window with 1500 changes sorted by name", () => sortedFiles(files, "name", false)],
    ["refs.toggle", "Space on 4000 selected tag rows including their folder", () => toggleSelected(selectionTree, selected, new Set())],
    ["commit.scope", "Commit What You See; 1500 staged files, *.svelte mask", () => commitScope(files, "*.svelte")],
    ["files.search", "filter 1500 changed files by name", () => files.filter((file) => matches(file, pattern))],
    ["scan.select", "Select All among 500 shown of 1000 discovered repositories", () => { scan.hits = hits; scan.chosen = new Set(); scan.toggleAll(shown); return scan.chosen; }],
    ["scan.receive", "receive a burst of 1000 repository discovery events", async () => { await scan.run("D:/work", 6); return scan.hits; }],
    ["files.sort", "Files: sort 1500 changes by path", () => sortRows(files, { key: "path", descending: false }, false)],
    ["diff.reveal", "20k-line full diff after 500 context expansions", () => foldDiff({ hunks: [full], oldTotal: 20000, newTotal: 20000, context: 3, revealed })],
    ["diff.headers", "diff of generated assets: 100 long lines, 752KB UTF-8 (below 1MB text limit)", () => foldDiff({ hunks: [longLines], oldTotal: 100, newTotal: 100, context: 3, revealed: [] })],
    ["diff.align", "switch a 20k-line full diff to Aligned", () => alignModel(model)],
    ["diff.highlight", "prepare syntax input for a revealed 20k-line diff", () => highlightedRows([full], () => entries, 1000)],
  ];
  const results = [];
  for (const [id, workload, run] of cases) {
    for (let i = 0; i < 5; i++) await run();
    const samples = [];
    let result;
    for (let i = 0; i < 31; i++) {
      const start = performance.now();
      result = await run();
      samples.push(performance.now() - start);
    }
    const sorted = [...samples].sort((a, b) => a - b);
    results.push({ id, workload, median: sorted[15], p95: sorted[29], samples, hash: hash(result) });
    expect(result).toBeDefined();
  }
  scan.clear();
  const dir = resolve(process.env.COGIT_USAGE_OUT ?? "../target/bench/results");
  mkdirSync(dir, { recursive: true });
  const label = process.env.COGIT_USAGE_BENCH!.replace(/[^a-zA-Z0-9_-]/g, "_");
  writeFileSync(resolve(dir, `usage-${label}.json`), JSON.stringify({ node: process.version, platform: platform(), cpu: cpus()[0]?.model, timestamp: new Date().toISOString(), results }, null, 2));
  console.table(results.map(({ id, median, p95 }) => ({ id, median, p95 })));
}, 120000);
