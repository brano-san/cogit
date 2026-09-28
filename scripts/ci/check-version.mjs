// Fails when a release tag and the versions committed in the manifests disagree.
// Usage: node scripts/ci/check-version.mjs <version>   (tag without the leading "v")
import fs from 'node:fs';

const want = process.argv[2];
if (!want || !/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(want)) {
  console.error(`::error::"${want}" is not a version like 1.2.3 or 1.2.3-rc.1`);
  process.exit(1);
}

const cargo = fs.readFileSync('Cargo.toml', 'utf8');
const workspaceVersion = cargo.match(/\[workspace\.package\][^[]*?\nversion\s*=\s*"([^"]+)"/)?.[1];
const json = (p) => JSON.parse(fs.readFileSync(p, 'utf8'));

const found = {
  'Cargo.toml': workspaceVersion,
  'package.json': json('package.json').version,
  'frontend/package.json': json('frontend/package.json').version,
};
const tauri = json('src-tauri/tauri.conf.json').version;
if (tauri !== undefined) found['src-tauri/tauri.conf.json'] = tauri;

const bad = Object.entries(found).filter(([, v]) => v !== want);
for (const [file, v] of bad) {
  console.error(`::error file=${file}::${file} has version ${v ?? '(none)'}, the tag says ${want}`);
}
if (bad.length) {
  console.error(`Bump every manifest to ${want} in a commit, then move the tag to it.`);
  process.exit(1);
}
console.log(`All manifests are at ${want}`);
