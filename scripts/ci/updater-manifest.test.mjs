import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { buildManifest, staleUpdaterFiles } from './updater-manifest.mjs';

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

test('the deb is keyed linux-x86_64-deb; an unsigned AppImage and the dmg add nothing', () => {
  const d = dist({
    'Cogit_1.2.3_amd64.deb': '', 'Cogit_1.2.3_amd64.deb.sig': 'sig-deb',
    'Cogit_1.2.3_amd64.AppImage': '', 'Cogit_1.2.3_aarch64.dmg': '',
  });
  const m = buildManifest(d, '1.2.3', BASE);
  assert.deepEqual(Object.keys(m.platforms), ['linux-x86_64-deb']);
  assert.equal(m.platforms['linux-x86_64-deb'].url, `${BASE}/Cogit_1.2.3_amd64.deb`);
});

test('a rerun that drops a signature or latest.json leaves them stale', () => {
  assert.deepEqual(
    staleUpdaterFiles(['latest.json', 'A-setup.exe', 'A-setup.exe.sig'], ['A-setup.exe']),
    ['A-setup.exe.sig', 'latest.json'],
  );
});

test('nothing is stale when the signature and latest.json are rebuilt, or the release is new', () => {
  const existing = ['latest.json', 'A-setup.exe', 'A-setup.exe.sig'];
  assert.deepEqual(staleUpdaterFiles(existing, ['A-setup.exe', 'A-setup.exe.sig', 'latest.json']), []);
  assert.deepEqual(staleUpdaterFiles([], ['A-setup.exe']), []);
});

test('an installer without a signature has no entry', () => {
  assert.equal(buildManifest(dist({ 'Cogit_1.2.3_x64.msi': '' }), '1.2.3', BASE), null);
});

test('darwin-aarch64 downloads the signed app archive', () => {
  const archive = 'Cogit_1.2.3_aarch64.app.tar.gz';
  const d = dist({ [archive]: '', [archive + '.sig']: 'sig-mac\n', 'Cogit_1.2.3_aarch64_notarized.dmg': '' });
  assert.deepEqual(buildManifest(d, '1.2.3', BASE)?.platforms, {
    'darwin-aarch64': { signature: 'sig-mac', url: BASE + '/' + archive },
  });
});

test('an updater signature alone cannot enable an unsigned Mac release', () => {
  const archive = 'Cogit_1.2.3_aarch64.app.tar.gz';
  const d = dist({ [archive]: '', [archive + '.sig']: 'sig-mac', 'Cogit_1.2.3_aarch64.dmg': '' });
  assert.equal(buildManifest(d, '1.2.3', BASE), null);
});

test('unsigned or wrong-architecture archives and signed disk images cannot update the app', () => {
  const d = dist({
    'Cogit_1.2.3_aarch64.app.tar.gz': '', 'Cogit_1.2.3_aarch64.app.tar.gz.sig': ' \n',
    'Cogit_1.2.3_x64.app.tar.gz': '', 'Cogit_1.2.3_x64.app.tar.gz.sig': 'sig-intel',
    'Cogit_1.2.3_aarch64.dmg': '', 'Cogit_1.2.3_aarch64.dmg.sig': 'sig-dmg',
  });
  assert.equal(buildManifest(d, '1.2.3', BASE), null);
});

test('a rerun cannot drop the signed Mac archive while rebuilding latest.json for Windows', () => {
  const archive = 'Cogit_1.2.3_aarch64.app.tar.gz';
  const d = ['Cogit_1.2.3_aarch64.dmg', 'latest.json'];
  assert.deepEqual(staleUpdaterFiles([...d, archive, archive + '.sig'], d), [archive]);
});

test('a rerun cannot leave the previous notarized disk image outside the new checksums', () => {
  const signed = 'Cogit_1.2.3_aarch64_notarized.dmg';
  assert.deepEqual(staleUpdaterFiles([signed], ['Cogit_1.2.3_aarch64.dmg']), [signed]);
});

test('a rerun whose experimental Mac build failed keeps the earlier Mac files', () => {
  const existing = ['Cogit_1.2.3_aarch64.dmg', 'Cogit_1.2.3_aarch64.app.tar.gz', 'Cogit_1.2.3_x64-setup.exe'];
  assert.deepEqual(staleUpdaterFiles(existing, ['Cogit_1.2.3_x64-setup.exe']), []);
});
