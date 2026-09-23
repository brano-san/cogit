// Benchmark repositories, deterministic (doc/15-benchmark.md): node scripts/bench/fixtures.mjs [--only small,large]

import { spawn } from "node:child_process";
import { mkdir, rm, writeFile, readFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { join, resolve } from "node:path";

const AUTHOR = "Cogit Bench <bench@cogit.test>";
const BASE_TIMESTAMP = 1_767_225_600;

const args = new Map();
for (let i = 2; i < process.argv.length; i += 2) {
  args.set(process.argv[i].replace(/^--/, ""), process.argv[i + 1]);
}
export const ROOT = resolve(args.get("root") ?? "target/bench/repos");
const ONLY = args.get("only")?.split(",");

const ENV = {
  ...process.env,
  GIT_TERMINAL_PROMPT: "0",
  LC_ALL: "C",
  GIT_CONFIG_NOSYSTEM: "1",
  GIT_AUTHOR_NAME: "Cogit Bench",
  GIT_AUTHOR_EMAIL: "bench@cogit.test",
  GIT_COMMITTER_NAME: "Cogit Bench",
  GIT_COMMITTER_EMAIL: "bench@cogit.test",
  GIT_AUTHOR_DATE: `${BASE_TIMESTAMP} +0000`,
  GIT_COMMITTER_DATE: `${BASE_TIMESTAMP} +0000`,
};

export function git(args, cwd, { input } = {}) {
  return new Promise((ok, fail) => {
    const child = spawn("git", args, { cwd, env: ENV, stdio: ["pipe", "pipe", "pipe"] });
    let out = "";
    let err = "";
    child.stdout.on("data", (chunk) => (out += chunk));
    child.stderr.on("data", (chunk) => (err += chunk));
    child.on("error", fail);
    child.on("close", (code) =>
      code === 0 ? ok(out.trim()) : fail(new Error(`git ${args.join(" ")} (${cwd}) exited ${code}\n${err}`)),
    );
    if (input !== undefined) child.stdin.end(input);
    else child.stdin.end();
  });
}

async function fresh(dir) {
  if (existsSync(dir)) await rm(dir, { recursive: true, force: true });
  await mkdir(dir, { recursive: true });
}

async function init(dir) {
  await fresh(dir);
  await git(["init", "--initial-branch=main", "--quiet"], dir);
  await git(["config", "user.name", "Cogit Bench"], dir);
  await git(["config", "user.email", "bench@cogit.test"], dir);
  await git(["config", "core.autocrlf", "false"], dir);
}

/** Feeds a fast-import stream in blocks, so a 50 000-commit history is never one string. */
async function fastImport(dir, blocks) {
  const child = spawn("git", ["fast-import", "--quiet"], { cwd: dir, env: ENV, stdio: ["pipe", "ignore", "pipe"] });
  let err = "";
  child.stderr.on("data", (chunk) => (err += chunk));
  const closed = new Promise((ok) => child.on("close", ok));
  for (const block of blocks) {
    if (!child.stdin.write(block)) await new Promise((ok) => child.stdin.once("drain", ok));
  }
  child.stdin.end();
  if ((await closed) !== 0) throw new Error(`fast-import failed in ${dir}\n${err}`);
}

const lines = (path, count, revision) =>
  Array.from({ length: count }, (_, k) =>
    k === revision % count ? `line ${k} of ${path}, revision ${revision}` : `line ${k} of ${path}`,
  ).join("\n") + "\n";

const blob = (path, text) => `M 100644 inline ${path}\ndata ${Buffer.byteLength(text)}\n${text}\n`;

/** ASCII only: `data <n>` counts bytes. */
function* history({ commits, files, fileLines, mergeEvery, sideLength, branches, tags, extra = [] }) {
  let mark = 0;
  let tick = 0;
  let mainTip = 0;
  const marks = [];
  const commit = (ref, parents, message, changes) => {
    mark += 1;
    tick += 1;
    const who = `${AUTHOR} ${BASE_TIMESTAMP + tick * 60} +0000`;
    let text = `commit ${ref}\nmark :${mark}\nauthor ${who}\ncommitter ${who}\ndata ${message.length}\n${message}\n`;
    if (parents[0]) text += `from :${parents[0]}\n`;
    for (const parent of parents.slice(1)) text += `merge :${parent}\n`;
    marks.push(mark);
    return text + changes + "\n";
  };

  let first = "";
  for (const path of files) first += blob(path, lines(path, fileLines, 0));
  for (const [path, text] of extra) first += blob(path, text);
  yield commit("refs/heads/main", [], "initial import", first);
  mainTip = mark;

  let block = "";
  for (let i = 1; i < commits; i += 1) {
    const path = files[i % files.length];
    if (mergeEvery && i % mergeEvery === 0) {
      let sideTip = mainTip;
      for (let s = 0; s < sideLength; s += 1) {
        const sidePath = files[(i + s * 7) % files.length];
        block += commit(`refs/heads/side/${i}`, [sideTip], `side ${i}.${s}`, blob(sidePath, lines(sidePath, fileLines, i * 10 + s)));
        sideTip = mark;
      }
      block += commit("refs/heads/main", [mainTip, sideTip], `Merge side ${i}`, "");
      mainTip = mark;
      block += `reset refs/heads/side/${i}\nfrom 0000000000000000000000000000000000000000\n\n`;
    }
    block += commit("refs/heads/main", [mainTip], `commit ${i}`, blob(path, lines(path, fileLines, i)));
    mainTip = mark;
    if (block.length > 1 << 20) {
      yield block;
      block = "";
    }
  }
  for (let b = 0; b < branches; b += 1) {
    block += `reset refs/heads/feature/${String(b).padStart(3, "0")}\nfrom :${marks[Math.floor(((b + 1) * marks.length) / (branches + 1))]}\n\n`;
  }
  for (let t = 0; t < tags; t += 1) {
    block += `reset refs/tags/v${Math.floor(t / 10)}.${t % 10}\nfrom :${marks[Math.floor(((t + 1) * marks.length) / (tags + 1))]}\n\n`;
  }
  yield block;
}

const tree = (count, dirs) => Array.from({ length: count }, (_, i) => `src/d${String(i % dirs).padStart(2, "0")}/file${String(i).padStart(4, "0")}.txt`);

async function build(dir, shape) {
  await init(dir);
  await fastImport(dir, history(shape));
  await git(["reset", "--hard", "main", "--quiet"], dir);
}

const BIG = Array.from({ length: 10_000 }, (_, k) => `big line ${k}: ${"x".repeat(k % 60)}`).join("\n") + "\n";
const binary = (seed) => {
  const bytes = Buffer.alloc(64 * 1024);
  let state = seed;
  for (let i = 0; i < bytes.length; i += 1) {
    state = (state * 1_103_515_245 + 12_345) >>> 0;
    bytes[i] = state >>> 24;
  }
  return bytes;
};

const SETS = {
  async small(root) {
    await build(join(root, "small"), { commits: 100, files: tree(20, 4), fileLines: 30, branches: 2, tags: 2 });
  },

  async medium(root) {
    const dir = join(root, "medium");
    await build(dir, { commits: 5000, files: tree(300, 30), fileLines: 40, mergeEvery: 250, sideLength: 10, branches: 12, tags: 25 });
    await git(["worktree", "add", "--quiet", join(root, "medium-worktree"), "feature/005"], dir);
  },

  async large(root) {
    await build(join(root, "large"), { commits: 50_000, files: tree(50, 10), fileLines: 8, mergeEvery: 500, sideLength: 6, branches: 300, tags: 100 });
  },

  async dirty(root) {
    const dir = join(root, "dirty");
    const files = tree(2000, 40);
    await build(dir, {
      commits: 300,
      files,
      fileLines: 30,
      branches: 2,
      tags: 2,
      extra: [
        ["big.txt", BIG],
        ["small.txt", "one\ntwo\nthree\n"],
      ],
    });
    await writeFile(join(dir, "image.bin"), binary(1));
    await git(["add", "image.bin"], dir);
    await git(["commit", "--quiet", "-m", "add a binary"], dir);
    for (let i = 0; i < 1500; i += 1) {
      const path = join(dir, files[i]);
      await writeFile(path, (await readFile(path, "utf8")) + `changed in the working tree ${i}\n`);
    }
    for (let i = 0; i < 500; i += 1) {
      await mkdir(join(dir, "new", `d${i % 10}`), { recursive: true });
      await writeFile(join(dir, "new", `d${i % 10}`, `untracked${i}.txt`), `untracked ${i}\n`);
    }
    await writeFile(join(dir, "big.txt"), BIG.split("\n").map((line, k) => (k % 100 === 0 ? `${line} changed` : line)).join("\n"));
    await writeFile(join(dir, "small.txt"), "one\ntwo changed\nthree\n");
    await writeFile(join(dir, "image.bin"), binary(2));
    // The first read of freshly written files costs ~9.5 s even for a bare `git add` (the
    // antivirus sees them once); read them now, so no measurement pays it.
    await git(["add", "--all"], dir);
    await git(["reset", "--quiet"], dir);
  },

  async submodules(root) {
    const upstream = join(root, "submodule-upstreams");
    await fresh(upstream);
    const make = async (name, commits, modules = []) => {
      const work = join(upstream, `${name}-work`);
      await build(work, { commits, files: tree(15, 3), fileLines: 20, branches: 1, tags: 1 });
      for (const [path, url] of modules) {
        await git(["-c", "protocol.file.allow=always", "submodule", "add", "--quiet", url, path], work);
      }
      if (modules.length) await git(["commit", "--quiet", "-m", "add submodules"], work);
      const bare = join(upstream, `${name}.git`);
      await git(["clone", "--quiet", "--bare", work, bare], upstream);
      return bare.replaceAll("\\", "/");
    };
    const proto = await make("proto", 40);
    const codec = await make("codec", 40);
    const libjam = await make("libjam", 60, [["submodules/proto", proto], ["submodules/codec", codec]]);
    const ewcore = await make("ewcore", 60, [["submodules/proto", proto]]);
    const leaves = [];
    for (let i = 0; i < 5; i += 1) leaves.push(await make(`leaf${i}`, 30));

    const parent = join(root, "submodules");
    await build(parent, { commits: 400, files: tree(60, 6), fileLines: 20, mergeEvery: 50, sideLength: 3, branches: 4, tags: 4 });
    await git(["config", "protocol.file.allow", "always"], parent);
    const modules = [["import/libjam", libjam], ["import/ewcore", ewcore], ...leaves.map((url, i) => [`import/leaf${i}`, url])];
    for (const [path, url] of modules) {
      await git(["-c", "protocol.file.allow=always", "submodule", "add", "--quiet", url, path], parent);
    }
    await git(["commit", "--quiet", "-m", "add submodules"], parent);
    await git(["-c", "protocol.file.allow=always", "submodule", "update", "--init", "--recursive", "--quiet"], parent);
    await git(["checkout", "--quiet", "HEAD~1"], join(parent, "import", "leaf0"));
  },

  /** A bare remote on the same disk, the clone the app works in, and a second clone that
      puts new commits on the remote between runs. */
  async network(root) {
    const seed = join(root, "network-seed");
    await build(seed, { commits: 2000, files: tree(200, 20), fileLines: 30, mergeEvery: 200, sideLength: 5, branches: 4, tags: 4 });
    const remote = join(root, "network-remote.git");
    await fresh(remote);
    await git(["clone", "--quiet", "--bare", seed, remote], root);
    await rm(seed, { recursive: true, force: true });
    for (const name of ["network", "network-pusher"]) {
      const dir = join(root, name);
      if (existsSync(dir)) await rm(dir, { recursive: true, force: true });
      await git(["clone", "--quiet", remote, dir], root);
      await git(["config", "user.name", "Cogit Bench"], dir);
      await git(["config", "user.email", "bench@cogit.test"], dir);
      await git(["config", "core.autocrlf", "false"], dir);
    }
  },
};

export const SET_NAMES = Object.keys(SETS);

export async function rebuild(names, root = ROOT) {
  await mkdir(root, { recursive: true });
  for (const name of names) {
    const make = SETS[name];
    if (!make) continue;
    if (name === "medium" && existsSync(join(root, "medium"))) {
      await git(["worktree", "prune"], join(root, "medium")).catch(() => {});
      await rm(join(root, "medium-worktree"), { recursive: true, force: true });
    }
    await make(root);
  }
}

async function main() {
  await mkdir(ROOT, { recursive: true });
  for (const [name, make] of Object.entries(SETS)) {
    if (ONLY && !ONLY.includes(name)) continue;
    const started = Date.now();
    process.stdout.write(`${name}… `);
    await rebuild([name]);
    console.log(`${((Date.now() - started) / 1000).toFixed(1)}s`);
  }
}

if (process.argv[1]?.endsWith("fixtures.mjs")) {
  await main();
}
