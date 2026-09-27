import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const keyPath = path.join(process.env.USERPROFILE, '.cogit', 'cogit-updater.key');
if (!fs.existsSync(keyPath)) {
  console.error(`Updater key not found at: ${keyPath}`);
  process.exit(1);
}

const key = fs.readFileSync(keyPath, 'utf8').trim();

const env = {
  ...process.env,
  TAURI_SIGNING_PRIVATE_KEY: key,
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD: '',
  CI: '1',
};

const npx = process.platform === 'win32' ? 'npx.cmd' : 'npx';
console.log('Starting tauri build with updater signing key...');
const res = spawnSync(npx, ['tauri', 'build', '--ci'], {
  env,
  stdio: 'inherit',
  shell: true,
});

process.exit(res.status ?? 0);
