// Compare the data-operation scenarios, refusing different workloads/results.
// node scripts/bench/usage-compare.mjs before.json after.json
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";

const [beforePath, afterPath] = process.argv.slice(2);
if (!beforePath || !afterPath) throw new Error("usage: usage-compare.mjs before.json after.json");
const before = JSON.parse(readFileSync(beforePath, "utf8"));
const after = JSON.parse(readFileSync(afterPath, "utf8"));
assert.equal(before.node, after.node, "Node versions differ");
assert.equal(before.cpu, after.cpu, "CPUs differ");
assert.equal(before.results.length, after.results.length, "Scenario counts differ");
const rows = before.results.map((was) => {
  const now = after.results.find((row) => row.id === was.id);
  assert.ok(now, `missing ${was.id}`);
  assert.equal(now.workload, was.workload, `workload changed: ${was.id}`);
  assert.equal(now.hash, was.hash, `result changed: ${was.id}`);
  const fixed = (value) => Number(value.toFixed(3));
  return { id: was.id, before: fixed(was.median), after: fixed(now.median), savedMs: fixed(was.median - now.median), speedup: fixed(was.median / now.median), p95Before: fixed(was.p95), p95After: fixed(now.p95) };
});
console.table(rows);
console.log("All scenario output hashes match. Times are data preparation, not end-to-end UI latency.");
