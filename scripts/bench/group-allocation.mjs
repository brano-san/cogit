// Isolates the grouping loop used by scan-groups, ref-selection and repo-menu.
import assert from 'node:assert/strict';

const values = Array.from({ length: 10_000 }, (_, i) => i);
function group(copy) {
  const groups = new Map();
  for (const value of values) {
    const key = 'one-folder';
    const entries = groups.get(key);
    if (copy) groups.set(key, [...(entries ?? []), value]);
    else if (entries) entries.push(value);
    else groups.set(key, [value]);
  }
  return groups.get('one-folder');
}
assert.deepEqual(group(true), group(false));
for (const copy of [true, false]) {
  for (let i = 0; i < 5; i++) group(copy);
  const start = performance.now();
  for (let i = 0; i < 30; i++) group(copy);
  console.log(`${copy ? 'copy' : 'push'}: ${(performance.now() - start) / 30} ms/call`);
}
