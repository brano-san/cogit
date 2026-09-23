import { spawnSync } from "node:child_process";
import { resolve } from "node:path";

const r = spawnSync("npm", ["run", "tauri", "--", "build", "--config", "src-tauri/tauri.bench.conf.json", "--no-bundle"], {
  stdio: "inherit",
  shell: true,
  env: { ...process.env, CARGO_TARGET_DIR: resolve("target/bench") },
});
process.exit(r.status ?? 1);
