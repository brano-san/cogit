// node scripts/bench/compare.mjs <before.json> <after.json> [--notes notes.json] [--md out.md]
// node scripts/bench/compare.mjs <ab.json> --ab before,after [--notes notes.json] [--md out.md]
//   The second form takes both columns from one A/B session — the same machine in the same
//   hour — and adds the 95 % interval of the ratio (doc/15-benchmark.md).

import { readFile, writeFile } from "node:fs/promises";

const args = process.argv.slice(2);
const flag = (name) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : undefined;
};
const files = args.filter((arg, i) => !arg.startsWith("--") && !args[i - 1]?.startsWith("--"));
const read = async (path) => JSON.parse(await readFile(path, "utf8"));

let before;
let after;
const intervals = new Map();
if (flag("ab") && files.length === 1) {
  const [a, b] = flag("ab").split(",");
  const session = await read(files[0]);
  before = { results: session.results.filter((r) => r.variant === a) };
  after = { results: session.results.filter((r) => r.variant === b) };
  for (const line of session.ab ?? []) intervals.set(line.key, line);
} else if (files.length === 2) {
  [before, after] = await Promise.all(files.map(read));
} else {
  console.error("usage: compare.mjs <before.json> <after.json> | <ab.json> --ab before,after [--notes notes.json] [--md out.md]");
  process.exit(2);
}

const notes = flag("notes") ? await read(flag("notes")) : {};
const key = (r) => `${r.id}|${r.set}|${r.condition}`;
const afterByKey = new Map(after.results.map((r) => [key(r), r]));

const fmt = (r) => (r && r.median !== null ? `${r.median} / ${r.p95}` : "—");
const verdicts = { faster: "быстрее", SLOWER: "медленнее", same: "без изменений" };
const rows = [...before.results]
  .filter((r) => r.median !== null)
  .sort((a, b) => b.median - a.median)
  .map((b) => {
    const a = afterByKey.get(key(b));
    const delta = a && a.median !== null ? a.median - b.median : null;
    const pct = delta !== null && b.median > 0 ? (100 * delta) / b.median : null;
    const line = intervals.get(`${b.id}|${b.set}`);
    const note = notes[key(b)] ?? notes[b.id] ?? (pct !== null && Math.abs(pct) < 10 ? "без изменений" : "");
    const condition = `${b.set}, ${b.condition === "cold" ? "холодный" : "тёплый"}`;
    const change = delta === null ? "—" : `${delta > 0 ? "+" : ""}${delta.toFixed(1)} мс (${pct > 0 ? "+" : ""}${pct.toFixed(0)} %)`;
    const interval = intervals.size ? ` ${line ? `[${line.lo}; ${line.hi}] ${verdicts[line.verdict]}` : "—"} |` : "";
    return `| ${b.group}: ${b.title} | ${condition} | ${fmt(b)} | ${fmt(a)} | ${change} |${interval} ${note} |`;
  });

const head = intervals.size
  ? ["| Действие | Условия | До (медиана / p95), мс | После (медиана / p95), мс | Δ | 95 % интервал отношения | Что изменено |", "|---|---|---|---|---|---|---|"]
  : ["| Действие | Условия | До (медиана / p95), мс | После (медиана / p95), мс | Δ | Что изменено |", "|---|---|---|---|---|---|"];
const table = [...head, ...rows].join("\n");

console.log(table);
if (flag("md")) await writeFile(flag("md"), table + "\n");
