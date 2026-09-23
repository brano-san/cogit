import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { join, resolve } from "node:path";

// `--keep <name>` also stores the build as target/bench/exes/<name>.exe, for --ab and --exe.
const keep = process.argv.indexOf("--keep");
const name = keep >= 0 ? process.argv[keep + 1] : null;

if (!process.argv.includes("--only-keep")) {
  const r = spawnSync("npm", ["run", "tauri", "--", "build", "--config", "src-tauri/tauri.bench.conf.json", "--no-bundle"], {
    stdio: "inherit",
    shell: true,
    env: { ...process.env, CARGO_TARGET_DIR: resolve("target/bench") },
  });
  if (r.status !== 0) process.exit(r.status ?? 1);
}
if (name) {
  mkdirSync(resolve("target/bench/exes"), { recursive: true });
  copyFileSync(resolve("target/bench/release/cogit.exe"), join(resolve("target/bench/exes"), `${name}.exe`));
  console.log(`kept as target/bench/exes/${name}.exe`);
}
