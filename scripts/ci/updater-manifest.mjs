// Builds latest.json, the manifest the app's updater endpoint reads, from the files in a dist folder.
// Usage: node scripts/ci/updater-manifest.mjs <dist> <version> <base-url>   (writes <dist>/latest.json)
//
// The updater looks for "{os}-{arch}-{installer}" first and falls back to the bare "{os}-{arch}",
// which would send an MSI copy to the NSIS installer; so only per-installer keys are written.
// An entry needs its installer's updater signature. Release notes stay empty: they are written
// by hand in the release (doc/12-risks.md).
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// platform key, installer suffix. The portable zip/AppImage and the dmg do not self-update.
const INSTALLERS = [
  ['windows-x86_64-nsis', '-setup.exe'],
  ['windows-x86_64-msi', '.msi'],
  ['linux-x86_64-deb', '.deb'],
];

/** The manifest for `dist`, or null when no installer there has a signature. */
export function buildManifest(dist, version, base, pubDate = new Date().toISOString()) {
  const files = fs.readdirSync(dist);
  const platforms = {};
  for (const [key, suffix] of INSTALLERS) {
    const file = files.find((f) => f.endsWith(suffix) && files.includes(`${f}.sig`));
    if (!file) continue;
    const signature = fs.readFileSync(path.join(dist, `${file}.sig`), 'utf8').trim();
    platforms[key] = { signature, url: `${base}/${file}` };
  }
  if (!Object.keys(platforms).length) return null;
  return { version, notes: '', pub_date: pubDate, platforms };
}

/**
 * Assets already in a release that `--clobber` upload of `dist` would leave out of step with the
 * new files: the signature of an installer that is replaced without a new one, and a latest.json
 * that is not rebuilt. Uploading cannot delete, so a rerun without the signing key must fail.
 */
export function staleUpdaterFiles(existing, dist) {
  const stale = existing.filter((name) => name.endsWith('.sig') && dist.includes(name.slice(0, -4)) && !dist.includes(name));
  if (existing.includes('latest.json') && !dist.includes('latest.json')) stale.push('latest.json');
  return stale;
}

if (process.argv[1] === fileURLToPath(import.meta.url) && process.argv[2] === 'stale') {
  // stale <dist>: the names of the existing release assets arrive on stdin, one per line.
  const existing = fs.readFileSync(0, 'utf8').split('\n').map((l) => l.trim()).filter(Boolean);
  const stale = staleUpdaterFiles(existing, fs.readdirSync(process.argv[3]));
  if (stale.length) {
    console.error(`::error::The release has ${stale.join(', ')}, which this run would leave stale. Restore TAURI_UPDATER_KEY and rerun.`);
    process.exit(1);
  }
} else if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [dist, version, base] = process.argv.slice(2);
  const manifest = buildManifest(dist, version, base);
  if (manifest) fs.writeFileSync(path.join(dist, 'latest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
  else console.log('::notice::No updater signature: latest.json is not published');
}
