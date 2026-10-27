import path from 'node:path';
import { spawnSync } from 'node:child_process';

const root = path.resolve(import.meta.dirname, '..');
const embedded = 'target/cogit-agent/cogit-agent';
const target = 'x86_64-unknown-linux-musl';
const args = process.argv.slice(2);
const release = args[0] === '--release';
const forTauri = release ? args.slice(1) : [];

function run(program, programArgs, options = {}) {
  const result = spawnSync(program, programArgs, { stdio: 'inherit', ...options });
  if (result.error) {
    console.error(`${program}: ${result.error.message}`);
    process.exit(1);
  }
  if (result.status !== 0) process.exit(result.status ?? 1);
}

const linux = process.platform !== 'win32';
const distro = process.env.COGIT_WSL_DISTRO ? ['-d', process.env.COGIT_WSL_DISTRO] : [];
const inside = linux
  ? root
  : spawnSync('wsl.exe', [...distro, '-e', 'wslpath', '-a', root.replaceAll('\\', '/')], {
      encoding: 'utf8',
    }).stdout?.trim();
if (!inside) {
  console.error('WSL did not translate the checkout path: is a distribution installed?');
  process.exit(1);
}

const build = [
  'set -e',
  'cd "$1"',
  'export RUSTC_WRAPPER=',
  'export CARGO_TARGET_DIR="${COGIT_MUSL_TARGET:-$HOME/cogit-musl}"',
  `cargo build --locked --release -p app_state --bin cogit-agent --target ${target}`,
  `mkdir -p "$(dirname "${embedded}")"`,
  `cp "$CARGO_TARGET_DIR/${target}/release/cogit-agent" "${embedded}"`,
  `ls -l "${embedded}"`,
].join('\n');

if (linux) run('bash', ['-lc', build, 'build-agent', inside]);
else run('wsl.exe', [...distro, '-e', 'bash', '-lc', build, 'build-agent', inside]);

if (release) {
  const npx = process.platform === 'win32' ? 'npx.cmd' : 'npx';
  run(npx, ['tauri', 'build', ...forTauri], {
    cwd: root,
    shell: process.platform === 'win32',
    env: { ...process.env, COGIT_AGENT_BIN: embedded, COGIT_REQUIRE_AGENT: '1' },
  });
}
