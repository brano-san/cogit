// node scripts/bench/compare.mjs <before.json> <after.json> [--notes notes.json] [--md out.md]

import { readFile, writeFile } from "node:fs/promises";

const [beforePath, afterPath, ...rest] = process.argv.slice(2);
const flag = (name) => {
  const i = rest.indexOf(`--${name}`);
  return i >= 0 ? rest[i + 1] : undefined;
};
if (!beforePath || !afterPath) {
  console.error("usage: node scripts/bench/compare.mjs <before.json> <after.json> [--notes notes.json] [--md out.md]");
  process.exit(2);
}

const before = JSON.parse(await readFile(beforePath, "utf8"));
const after = JSON.parse(await readFile(afterPath, "utf8"));
const notes = flag("notes") ? JSON.parse(await readFile(flag("notes"), "utf8")) : {};
const key = (r) => `${r.id}|${r.set}|${r.condition}`;
const afterByKey = new Map(after.results.map((r) => [key(r), r]));

const fmt = (r) => (r && r.median !== null ? `${r.median} / ${r.p95}` : "—");
const rows = [...before.results]
  .filter((r) => r.median !== null)
  .sort((a, b) => b.median - a.median)
  .map((b) => {
    const a = afterByKey.get(key(b));
    const delta = a && a.median !== null ? a.median - b.median : null;
    const pct = delta !== null && b.median > 0 ? (100 * delta) / b.median : null;
    const note = notes[key(b)] ?? notes[b.id] ?? (pct !== null && Math.abs(pct) < 10 ? "без изменений" : "");
    const condition = `${b.set}, ${b.condition === "cold" ? "холодный" : "тёплый"}`;
    const change = delta === null ? "—" : `${delta > 0 ? "+" : ""}${delta.toFixed(1)} мс (${pct > 0 ? "+" : ""}${pct.toFixed(0)} %)`;
    return `| ${b.group}: ${b.title} | ${condition} | ${fmt(b)} | ${fmt(a)} | ${change} | ${note} |`;
  });

const table = [
  "| Действие | Условия | До (медиана / p95), мс | После (медиана / p95), мс | Δ | Что изменено |",
  "|---|---|---|---|---|---|",
  ...rows,
].join("\n");

console.log(table);
if (flag("md")) await writeFile(flag("md"), table + "\n");
