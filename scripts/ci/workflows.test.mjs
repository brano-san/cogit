// Rules for .github/workflows/*.yml and .github/actions/**/action.yml (doc/12-risks.md, BR-03):
// third-party actions are pinned by commit SHA, and secrets are read only in a step's `env:`.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const root = path.join(import.meta.dirname, '..', '..', '.github');

function files() {
  const out = [];
  for (const f of fs.readdirSync(path.join(root, 'workflows'))) {
    if (f.endsWith('.yml')) out.push(path.join(root, 'workflows', f));
  }
  const walk = (dir) => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      if (e.isDirectory()) walk(path.join(dir, e.name));
      else if (e.name === 'action.yml') out.push(path.join(dir, e.name));
    }
  };
  walk(path.join(root, 'actions'));
  return out;
}

const indent = (line) => line.length - line.trimStart().length;

test('every remote action is pinned to a full commit SHA', () => {
  for (const file of files()) {
    for (const line of fs.readFileSync(file, 'utf8').split('\n')) {
      const m = line.match(/^\s*(?:-\s+)?uses:\s*(\S+)/);
      if (!m || m[1].startsWith('./')) continue;
      assert.match(m[1], /@[0-9a-f]{40}$/, `${path.basename(file)}: ${m[1]} is not pinned by SHA`);
    }
  }
});

// A secret may be named only in a step's `env:` block; a job's `env:` holds only `secrets.X != ''`.
test('secrets reach only step env blocks, job env holds presence checks', () => {
  for (const file of files()) {
    const lines = fs.readFileSync(file, 'utf8').split('\n');
    let envIndent = -1; // indent of the `env:` key we are inside, or -1
    let inStep = false; // the current `env:` belongs to a step (`- ` item) rather than a job
    let stepIndent = -1;
    lines.forEach((line, i) => {
      if (/^\s*-\s/.test(line)) {
        stepIndent = indent(line);
        inStep = true;
      }
      if (/^\s{0,5}\w[\w-]*:\s*$/.test(line) && indent(line) <= 2) inStep = false; // a new job
      if (/^\s*env:\s*$/.test(line)) {
        envIndent = indent(line);
        return;
      }
      if (envIndent >= 0 && line.trim() !== '' && indent(line) <= envIndent) envIndent = -1;
      if (!/\$\{\{[^}]*secrets\./.test(line)) return;
      const where = `${path.basename(file)}:${i + 1}`;
      const inEnv = envIndent >= 0;
      const stepEnv = inEnv && envIndent > stepIndent && inStep;
      const presence = /secrets\.\w+\s*!=\s*''/.test(line);
      assert.ok(stepEnv || (inEnv && presence), `${where}: secret outside a step env: ${line.trim()}`);
    });
  }
});
