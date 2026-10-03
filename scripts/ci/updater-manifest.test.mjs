import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { buildManifest } from './updater-manifest.mjs';

const BASE = 'https://example.test/dl';

function dist(files) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'manifest-'));
  for (const [name, body] of Object.entries(files)) fs.writeFileSync(path.join(dir, name), body);
  return dir;
}

test('MSI and NSIS get separate keys and no bare windows-x86_64', () => {
  const d = dist({
    'Cogit_1.2.3_x64.msi': '', 'Cogit_1.2.3_x64.msi.sig': 'sig-msi\n',
    'Cogit_1.2.3_x64-setup.exe': '', 'Cogit_1.2.3_x64-setup.exe.sig': 'sig-nsis\n',
  });
  const m = buildManifest(d, '1.2.3', BASE, 'D');
  assert.deepEqual(Object.keys(m.platforms).sort(), ['windows-x86_64-msi', 'windows-x86_64-nsis']);
  assert.deepEqual(m.platforms['windows-x86_64-msi'], { signature: 'sig-msi', url: `${BASE}/Cogit_1.2.3_x64.msi` });
  assert.deepEqual(m.platforms['windows-x86_64-nsis'], { signature: 'sig-nsis', url: `${BASE}/Cogit_1.2.3_x64-setup.exe` });
  assert.equal(m.notes, '');
  assert.equal(m.pub_date, 'D');
});

test('without an MSI only the NSIS key is written', () => {
  const d = dist({ 'Cogit_1.2.3_x64-setup.exe': '', 'Cogit_1.2.3_x64-setup.exe.sig': 's' });
  assert.deepEqual(Object.keys(buildManifest(d, '1.2.3', BASE).platforms), ['windows-x86_64-nsis']);
});

test('an installer without a signature has no entry', () => {
  assert.equal(buildManifest(dist({ 'Cogit_1.2.3_x64.msi': '' }), '1.2.3', BASE), null);
});
