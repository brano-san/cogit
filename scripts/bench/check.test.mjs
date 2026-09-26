// node --test "scripts/bench/*.test.mjs"
import { test } from "node:test";
import assert from "node:assert/strict";
import { overBudget } from "./check.mjs";

const budget = { id: "graph.full-layout", set: "large", condition: "warm", median: 346 };
const row = (fields) => ({ id: budget.id, set: budget.set, condition: budget.condition, median: 300, errors: [], ...fields });
const all = () => true;

test("a median within the budget passes", () => {
  assert.deepEqual(overBudget([row({})], [budget], all), []);
});

test("a median over the budget fails", () => {
  assert.deepEqual(overBudget([row({ median: 400 })], [budget], all), ["graph.full-layout · large · warm: 400 ms > 346 ms"]);
});

test("a scenario that only failed is not a met budget", () => {
  const failed = row({ median: null, errors: ["nothing matches button.row.header"] });
  assert.equal(overBudget([failed], [budget], all).length, 1);
});

test("a scenario with errors among its samples fails", () => {
  assert.equal(overBudget([row({ errors: ["the push did not reach the remote"] })], [budget], all).length, 1);
});

test("a budget the run should have measured but did not fails", () => {
  assert.deepEqual(overBudget([], [budget], all), ["graph.full-layout · large · warm: not measured"]);
});

test("a budget outside the run is not asked for", () => {
  assert.deepEqual(overBudget([], [budget], () => false), []);
});
