// Builds a repository with N commits for the OOM load test.
//
// The shape and the author/timestamp constants mirror `test_fixtures::stress`, so the
// figures this produces line up with the Rust suite. `fast-import` is what makes it
// bearable: two git processes per commit is what made the old fixtures slow (R-56).
//
//   node scripts/oom/make-repo.mjs <path> [commits]

import { spawn } from "node:child_process";
import { mkdir, rm } from "node:fs/promises";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

const AUTHOR_NAME = "Cogit Fixture";
const AUTHOR_EMAIL = "fixture@cogit.test";
const BASE_TIMESTAMP = 1_767_225_600;
const STEP_SECONDS = 60;

const target = resolve(process.argv[2] ?? "");
const commits = Number(process.argv[3] ?? 100_000);

if (!process.argv[2] || !Number.isInteger(commits) || commits < 1) {
  console.error("usage: node scripts/oom/make-repo.mjs <path> [commits]");
  process.exit(2);
}

function git(args, cwd, { stdin = false } = {}) {
  return new Promise((ok, fail) => {
    const child = spawn("git", args, {
      cwd,
      stdio: [stdin ? "pipe" : "ignore", "ignore", "pipe"],
      env: { ...process.env, GIT_TERMINAL_PROMPT: "0", LC_ALL: "C" },
    });
    let stderr = "";
    child.stderr.on("data", (chunk) => (stderr += chunk));
    child.on("error", fail);
    child.on("close", (code) =>
      code === 0 ? ok(child) : fail(new Error(`git ${args.join(" ")} exited ${code}\n${stderr}`)),
    );
    if (stdin) ok(child);
  });
}

/** One commit of the stream. `data <n>` counts bytes, which is why the payload is ASCII. */
function commitBlock(i) {
  const message = `commit ${i}`;
  const content = `revision ${i}\n`;
  const stamp = BASE_TIMESTAMP + i * STEP_SECONDS;
  const who = `${AUTHOR_NAME} <${AUTHOR_EMAIL}> ${stamp} +0000`;

  return (
    `commit refs/heads/main\n` +
    `mark :${i + 1}\n` +
    `author ${who}\n` +
    `committer ${who}\n` +
    `data ${message.length}\n${message}\n` +
    (i > 0 ? `from :${i}\n` : "") +
    `M 100644 inline data.txt\n` +
    `data ${content.length}\n${content}\n`
  );
}

async function main() {
  if (existsSync(target)) {
    console.log(`removing the previous ${target}`);
    await rm(target, { recursive: true, force: true });
  }
  await mkdir(target, { recursive: true });

  await git(["init", "--initial-branch=main", "--quiet"], target);
  await git(["config", "user.name", AUTHOR_NAME], target);
  await git(["config", "user.email", AUTHOR_EMAIL], target);

  console.log(`importing ${commits} commits into ${target}`);
  const started = Date.now();

  const child = spawn("git", ["fast-import", "--quiet"], {
    cwd: target,
    stdio: ["pipe", "ignore", "pipe"],
    env: { ...process.env, LC_ALL: "C" },
  });
  let stderr = "";
  child.stderr.on("data", (chunk) => (stderr += chunk));

  // Written in blocks: one 20 MB string for 100k commits is a needless spike in a script
  // whose whole subject is memory.
  const BLOCK = 2000;
  for (let from = 0; from < commits; from += BLOCK) {
    let block = "";
    for (let i = from; i < Math.min(from + BLOCK, commits); i += 1) block += commitBlock(i);
    if (!child.stdin.write(block)) {
      await new Promise((ok) => child.stdin.once("drain", ok));
    }
  }
  child.stdin.end();

  const code = await new Promise((ok) => child.on("close", ok));
  if (code !== 0) {
    console.error(stderr);
    process.exit(1);
  }

  await git(["reset", "--hard", "main", "--quiet"], target);
  console.log(`done in ${((Date.now() - started) / 1000).toFixed(1)}s`);
}

await main();
