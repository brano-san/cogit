// The benchmark: every scenario, cold and warm, on every repository set, against the
// release build. See doc/15-benchmark.md.
//
//   npm run bench -- [--label baseline] [--sets small,dirty] [--only repo.open,diff.big]
//                    [--runs 10] [--warmup 2] [--cold 10] [--no-cold] [--no-warm] [--no-app]
//                    [--check]
//
// Needs the bench build (npm run bench:build) and the repositories (npm run bench:repos).

import { spawn, spawnSync } from "node:child_process";
import { appendFile, mkdir, readFile, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import { join, resolve } from "node:path";
import { EXE, dropScript, launch, resetProfile, stop } from "./app.mjs";
import { KEYS, sleep } from "./cdp.mjs";
import { SCENARIOS } from "./scenarios.mjs";
import { BUDGETS } from "./budgets.mjs";
import { rebuild } from "./fixtures.mjs";

const args = new Map();
for (let i = 2; i < process.argv.length; i += 1) {
  const key = process.argv[i].replace(/^--/, "");
  const next = process.argv[i + 1];
  if (next === undefined || next.startsWith("--")) args.set(key, true);
  else {
    args.set(key, next);
    i += 1;
  }
}

const LABEL = String(args.get("label") ?? "run");
const RUNS = Number(args.get("runs") ?? 10);
const WARMUP = Number(args.get("warmup") ?? 2);
const COLD = Number(args.get("cold") ?? 10);
const ONLY = args.has("only") ? String(args.get("only")).split(",") : null;
const SETS = args.has("sets") ? String(args.get("sets")).split(",") : ["small", "medium", "large", "submodules", "dirty", "network"];
const REPOS = resolve("target/bench/repos");
const OUT = resolve(String(args.get("out") ?? `target/bench/results/${LABEL}.json`));
const LOG = OUT.replace(/\.json$/, ".log");

const wanted = (id) => !ONLY || ONLY.some((o) => id === o || id.startsWith(`${o}.`) || o === id.split(".")[0]);

async function note(line) {
  console.log(line);
  await appendFile(LOG, `${new Date().toISOString()} ${line}\n`);
}

// --- git, outside the clock ------------------------------------------------------------

function git(argv, cwd) {
  const r = spawnSync("git", argv, { cwd, encoding: "utf8", env: { ...process.env, GIT_TERMINAL_PROMPT: "0" } });
  if (r.status !== 0) throw new Error(`git ${argv.join(" ")} in ${cwd}: ${r.stderr}`);
  return r.stdout.trim();
}

class Git {
  constructor(ctx) {
    this.ctx = ctx;
  }
  get dir() {
    return this.ctx.repo;
  }
  /** The watcher debounces; give it time to notice, then wait for the app to settle. */
  async settle() {
    await sleep(700);
    await this.ctx.cdp.eval("window.__bench.idle(300, 30000)");
  }
  async touch(path) {
    const file = join(this.dir, path);
    await appendFile(file, `bench ${Date.now()}\n`);
  }
  async modify(path) {
    await this.touch(path);
    await this.settle();
  }
  async stage(path) {
    await this.touch(path);
    git(["add", path], this.dir);
    await this.settle();
  }
  async restore(path) {
    git(["reset", "-q", "--", path], this.dir);
    git(["checkout", "--", path], this.dir);
    await this.settle();
  }
  async undoCommit(path) {
    git(["reset", "-q", "--soft", "HEAD~1"], this.dir);
    await this.restore(path);
  }
  async stashPop(path) {
    git(["stash", "pop", "-q"], this.dir);
    await this.restore(path);
  }
  /** A new commit on the remote, pushed from the second clone. */
  async remoteCommit() {
    const pusher = join(REPOS, "network-pusher");
    git(["fetch", "-q", "origin"], pusher);
    git(["reset", "-q", "--hard", "origin/main"], pusher);
    await appendFile(join(pusher, "src/d00/file0000.txt"), `remote ${Date.now()}\n`);
    git(["commit", "-q", "-am", "remote change"], pusher);
    git(["push", "-q", "origin", "main"], pusher);
  }
  async localCommit() {
    await this.touch("src/d01/file0001.txt");
    git(["commit", "-q", "-am", "local change"], this.dir);
    await this.settle();
  }
}

// --- the context a scenario gets -------------------------------------------------------

const optionsSource = ({ until, probes } = {}) => {
  const parts = [];
  if (until) parts.push(`until: ${JSON.stringify(until)}`);
  if (probes) parts.push(`probes: { ${Object.entries(probes).map(([k, v]) => `${k}: () => (${v})`).join(", ")} }`);
  return `{ ${parts.join(", ")} }`;
};

class Context {
  constructor(app, set) {
    this.app = app;
    this.cdp = app.cdp;
    this.set = set;
    this.repo = join(REPOS, set).replaceAll("\\", "/");
    this.iteration = 0;
    this.git = new Git(this);
    this.measure = this.#actions();
    this.prep = this.#actions();
  }

  #actions() {
    const cdp = this.cdp;
    const point = async (spec) => {
      const at = await cdp.eval(`window.__bench.point(${JSON.stringify(spec)})`);
      if (!at) throw new Error(`nothing matches ${JSON.stringify(spec)}`);
      return at;
    };
    const input = async (send, opts = {}) => {
      const quiet = opts.quiet ?? 150;
      await cdp.eval(`window.__bench.arm(${quiet})`);
      await send();
      await cdp.park();
      return cdp.eval(`window.__bench.done(${quiet}, ${opts.timeout ?? 60000}, ${optionsSource(opts)})`);
    };
    const run = (script, opts = {}) =>
      cdp.eval(
        `window.__bench.run(async () => { await (${script}); }, ${opts.quiet ?? 150}, ${opts.timeout ?? 60000}, ${optionsSource(opts)})`,
      );
    return {
      click: async (spec, opts) => {
        const at = await point(spec);
        return input(() => cdp.click(at.x, at.y), opts);
      },
      doubleClick: async (spec, opts) => {
        const at = await point(spec);
        return input(() => cdp.doubleClick(at.x, at.y), opts);
      },
      rightClick: async (spec, opts) => {
        const at = await point(spec);
        return input(() => cdp.click(at.x, at.y, { button: "right" }), opts);
      },
      key: (name, opts = {}) => input(() => cdp.key(KEYS[name].key, { ...KEYS[name], modifiers: opts.modifiers ?? 0 }), opts),
      input: (selector, value, opts) =>
        run(
          `(() => { const el = document.querySelector(${JSON.stringify(selector)}); el.focus(); el.value = ${JSON.stringify(value)}; el.dispatchEvent(new Event("input", { bubbles: true })); })()`,
          opts,
        ),
      select: (selector, value, opts) =>
        run(
          `(() => { const el = [...document.querySelectorAll(${JSON.stringify(selector)})].find((s) => [...s.options].some((o) => o.value === ${JSON.stringify(value)})); el.value = ${JSON.stringify(value)}; el.dispatchEvent(new Event("change", { bubbles: true })); })()`,
          opts,
        ),
      menu: (id, opts) => run(`window.__bench.emit("menu-command", ${JSON.stringify(id)})`, opts),
      run,
      focus: async (selector) => {
        await cdp.eval(`document.querySelector(${JSON.stringify(selector)}).focus()`);
      },
      type: async (text) => {
        await cdp.type(text);
        await cdp.eval("window.__bench.idle(150, 10000)");
      },
    };
  }

  exists(spec) {
    return this.cdp.eval(`window.__bench.exists(${JSON.stringify(spec)})`);
  }
  attr(spec, name) {
    return this.cdp.eval(`window.__bench.attr(${JSON.stringify(spec)}, ${JSON.stringify(name)})`);
  }
  dropScript(path) {
    return dropScript(path);
  }
  async ensureOpen(set) {
    const present = await this.exists({ sel: ".wrapper .row", text: set, exact: ".name" });
    if (!present) await this.prep.run(dropScript(join(REPOS, set).replaceAll("\\", "/")), { quiet: 300 });
  }
  searchScript(query) {
    return `(async () => {
      const I = window.__TAURI_INTERNALS__;
      const norm = (p) => p.replaceAll("\\\\", "/").toLowerCase().replace(/\\/$/, "");
      const repos = await I.invoke("repositories");
      const repo = (repos.find((r) => norm(r.root) === norm(${JSON.stringify(this.repo)})) ?? repos[0]).repo;
      const channel = I.transformCallback(() => {}, false);
      await I.invoke("search_file_contents", { repo, query: ${JSON.stringify(query)}, isRegex: false, scope: "all", onChunk: "__CHANNEL__:" + channel });
    })()`;
  }
  /** The commit menu is a native popup (class #32768): Escape is posted to that window of
      our own process only, never typed into whatever has the focus. */
  async dismissNativeMenu() {
    const script = `
Add-Type @"
using System; using System.Runtime.InteropServices; using System.Text;
public static class BenchMenu {
  public delegate bool Enum(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(Enum f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  public static int Close(uint owner) {
    int closed = 0;
    EnumWindows((h, l) => {
      uint pid; GetWindowThreadProcessId(h, out pid);
      var name = new StringBuilder(64); GetClassName(h, name, 64);
      if (pid == owner && name.ToString() == "#32768") {
        PostMessage(h, 0x0100, (IntPtr)0x1B, IntPtr.Zero);
        PostMessage(h, 0x0101, (IntPtr)0x1B, IntPtr.Zero);
        closed++;
      }
      return true;
    }, IntPtr.Zero);
    return closed;
  }
}
"@
[BenchMenu]::Close(${this.app.child.pid})`;
    for (let attempt = 0; attempt < 5; attempt += 1) {
      await sleep(150);
      const r = spawnSync("powershell", ["-NoProfile", "-Command", script], { encoding: "utf8" });
      if (Number(r.stdout.trim()) > 0) break;
    }
    await this.cdp.eval("window.__bench.idle(200, 15000)");
  }
}

// --- statistics ------------------------------------------------------------------------

const quantile = (sorted, q) => sorted[Math.min(sorted.length - 1, Math.max(0, Math.ceil(q * sorted.length) - 1))];

function summarise(samples) {
  const totals = samples.map((s) => s.total).sort((a, b) => a - b);
  const ipcs = samples.map((s) => s.ipc ?? 0).sort((a, b) => a - b);
  const fronts = samples.map((s) => s.front ?? Math.max(s.total - (s.ipc ?? 0), 0)).sort((a, b) => a - b);
  const median = quantile(totals, 0.5);
  const p95 = quantile(totals, 0.95);
  return {
    n: totals.length,
    median: round(median),
    p95: round(p95),
    min: round(totals[0]),
    max: round(totals.at(-1)),
    ipcMedian: round(quantile(ipcs, 0.5)),
    frontMedian: round(quantile(fronts, 0.5)),
    spread: median > 0 ? round((p95 - median) / median, 3) : 0,
  };
}
const round = (v, digits = 1) => (v === undefined || Number.isNaN(v) ? null : Number(v.toFixed(digits)));

function commands(samples) {
  const sum = {};
  for (const s of samples) {
    for (const [cmd, c] of Object.entries(s.byCommand ?? {})) {
      const e = (sum[cmd] ??= { count: 0, ms: 0 });
      e.count += c.count;
      e.ms += c.ms;
    }
  }
  return Object.fromEntries(
    Object.entries(sum)
      .map(([cmd, e]) => [cmd, { perRun: round(e.count / samples.length, 1), msPerRun: round(e.ms / samples.length) }])
      .sort((a, b) => b[1].msPerRun - a[1].msPerRun)
      .slice(0, 12),
  );
}

// --- results ---------------------------------------------------------------------------

const results = new Map();
function record(id, set, condition, sample, meta) {
  const key = `${id}|${set}|${condition}`;
  if (!results.has(key)) results.set(key, { id, set, condition, ...meta, samples: [], errors: [] });
  results.get(key).samples.push(sample);
}
function fail(id, set, condition, error, meta) {
  const key = `${id}|${set}|${condition}`;
  if (!results.has(key)) results.set(key, { id, set, condition, ...meta, samples: [], errors: [] });
  results.get(key).errors.push(String(error.message ?? error).slice(0, 300));
}

const SCENARIO_LIMIT_MS = 180_000;

class Hung extends Error {}

function limited(promise, ms, what) {
  let timer;
  return Promise.race([
    promise.finally(() => clearTimeout(timer)),
    new Promise((_, fail) => {
      timer = setTimeout(() => fail(new Hung(`${what} hung for ${ms / 1000}s`)), ms);
    }),
  ]);
}

/** Returns false when the app had to be killed; the caller then starts a new one. */
async function once(scenario, ctx, condition, keep) {
  try {
    await limited(attempt(scenario, ctx, condition, keep), SCENARIO_LIMIT_MS, scenario.id);
    return true;
  } catch (error) {
    fail(scenario.id, ctx.set, condition, error, { group: scenario.group, title: scenario.title });
    await note(`  ! ${scenario.id} on ${ctx.set} (${condition}): ${String(error.message).split("\n")[0]}`);
    ctx.app.child.kill();
    return false;
  }
}

async function attempt(scenario, ctx, condition, keep) {
  const meta = { group: scenario.group, title: scenario.title };
  if (args.has("trace")) await note(`  > ${scenario.id} (${ctx.set}, ${condition}, #${ctx.iteration})`);
  try {
    await scenario.prep?.(ctx);
    const r = await scenario.measure(ctx);
    const named = r && typeof r.total === "number" ? { [scenario.id]: r } : r;
    for (const [id, sample] of Object.entries(named)) {
      if (sample.timedOut) throw new Error(`${id} timed out`);
      if (keep) record(id, ctx.set, condition, sample, id === scenario.id ? meta : { group: scenario.group, title: id });
    }
    await scenario.reset?.(ctx);
  } catch (error) {
    fail(scenario.id, ctx.set, condition, error, meta);
    await note(`  ! ${scenario.id} on ${ctx.set} (${condition}): ${String(error.message).split("\n")[0]}`);
    // Whatever state the failure left, the next scenario must not inherit a dialog.
    await ctx.cdp.key("Escape", { ...KEYS.Escape }).catch(() => {});
    await ctx.cdp.eval("window.__bench.idle(300, 15000)").catch(() => {});
  }
}

const applicable = (set) => SCENARIOS.filter((s) => s.sets.includes(set) && wanted(s.id));

async function openRepo(ctx) {
  await ctx.prep.run(dropScript(ctx.repo), { quiet: 300, timeout: 120000 });
}

async function exitApp(app) {
  await app.cdp.eval(`window.__bench.emit("menu-command", "exit")`).catch(() => {});
  await stop(app);
}

const errorsIn = (set, condition) =>
  [...results.values()].filter((r) => r.set === set && r.condition === condition).reduce((n, r) => n + r.errors.length, 0);

async function coldPass(set) {
  const list = applicable(set);
  if (list.length === 0) return;
  let before = errorsIn(set, "cold");
  for (let run = 0; run < WARMUP + COLD; run += 1) {
    // The repository is rebuilt only when the run before left it in doubt.
    if (!args.has("keep-repos") && run > 0 && errorsIn(set, "cold") > before) await rebuild([set]);
    before = errorsIn(set, "cold");
    await resetProfile();
    let app;
    try {
      app = await launch();
    } catch (error) {
      await note(`  ! launch failed (cold ${set} #${run}): ${error.message}`);
      continue;
    }
    const ctx = new Context(app, set);
    ctx.iteration = run;
    if (!list.some((s) => s.id === "repo.open")) {
      try {
        await limited(openRepo(ctx), 120_000, "open");
      } catch (error) {
        await note(`  ! open failed (cold ${set} #${run}): ${error.message}`);
        app.child.kill();
        continue;
      }
    }
    for (const scenario of list) {
      if (!(await once(scenario, ctx, "cold", run >= WARMUP))) break;
    }
    await exitApp(ctx.app);
    await note(`  cold ${set} ${run + 1}/${WARMUP + COLD}`);
  }
}

async function warmPass(set) {
  const list = applicable(set);
  if (list.length === 0) return;
  await resetProfile();
  if (args.has("trace")) await note("  . profile reset");
  const app = await launch();
  if (args.has("trace")) await note(`  . launched pid ${app.child.pid}`);
  const ctx = new Context(app, set);
  await openRepo(ctx);
  if (args.has("trace")) await note("  . repository open");
  for (const scenario of list) {
    const runs = scenario.runs ?? RUNS;
    for (let i = 0; i < WARMUP + runs; i += 1) {
      ctx.iteration = i;
      if (await once(scenario, ctx, "warm", i >= WARMUP)) continue;
      await stop(ctx.app, { gracefulMs: 2000 }).catch(() => {});
      const fresh = await launch();
      Object.assign(ctx, new Context(fresh, set), { iteration: i });
      await openRepo(ctx);
      break;
    }
    await note(`  warm ${set} ${scenario.id}: ${summarise(results.get(`${scenario.id}|${set}|warm`)?.samples ?? [{ total: NaN }]).median} ms`);
  }
  await exitApp(app);
}

/** Launch to first paint, launch to ready (a repository restored), and exit. */
async function appPass() {
  if (!wanted("app")) return;
  const medium = join(REPOS, "medium").replaceAll("\\", "/");
  const meta = (title) => ({ group: "Приложение", title });

  for (let run = 0; run < WARMUP + COLD; run += 1) {
    const keep = run >= WARMUP;
    await resetProfile();
    const app = await launch();
    const paint = await app.cdp.eval(`(async () => {
      for (let i = 0; i < 600; i += 1) {
        const e = performance.getEntriesByName("first-contentful-paint")[0];
        if (e) return performance.timeOrigin + e.startTime;
        await new Promise((ok) => setTimeout(ok, 25));
      }
      return null;
    })()`);
    if (keep && paint) record("app.first-paint", "empty", "cold", { total: paint - app.spawnedAt }, meta("запуск до первой отрисовки"));
    await app.cdp.eval(`window.__bench.idle(300, 20000)`);
    // The app writes its own session on the way out, so the repository goes in the way a
    // user puts it there: opened, then remembered.
    await app.cdp.eval(`window.__bench.run(() => ${dropScript(medium)}, 300, 60000)`);
    await exitApp(app);

    const restored = await launch();
    const ready = await restored.cdp.eval(`(async () => {
      const frame = () => new Promise((ok) => requestAnimationFrame(() => ok(performance.now())));
      let painted = null;
      for (let i = 0; i < 3000; i += 1) {
        const at = await frame();
        const shown = document.querySelectorAll('[role="listitem"]').length > 0 && document.querySelectorAll('[role="treeitem"]').length > 0;
        const s = window.__bench.state;
        const busy = !shown || s.inflight > 0 || s.dirty;
        s.dirty = false;
        if (busy) painted = null;
        else if (painted === null) painted = at;
        if (!busy && at - s.lastActivity >= 300) return performance.timeOrigin + painted;
      }
      return null;
    })()`);
    if (keep && ready) record("app.ready", "medium", "cold", { total: ready - restored.spawnedAt }, meta("запуск до готовности (medium восстановлен)"));

    const asked = Date.now();
    await restored.cdp.eval(`window.__bench.emit("menu-command", "exit")`).catch(() => {});
    const gone = await Promise.race([restored.exited, sleep(15000).then(() => null)]);
    if (keep && gone) record("app.close", "medium", "cold", { total: gone - asked }, meta("закрытие приложения"));
    await stop(restored);
    await note(`  app ${run + 1}/${WARMUP + COLD}`);
  }
}

// --- environment -----------------------------------------------------------------------

function ps(command) {
  const r = spawnSync("powershell", ["-NoProfile", "-Command", command], { encoding: "utf8" });
  return r.stdout.trim();
}

async function environment() {
  const cargo = await readFile("src-tauri/Cargo.toml", "utf8");
  const exe = await stat(EXE);
  const drive = resolve(".").slice(0, 1);
  return {
    label: LABEL,
    date: new Date().toISOString(),
    version: /^version\s*=\s*"([^"]+)"/m.exec(cargo)?.[1],
    commit: git(["rev-parse", "--short", "HEAD"], "."),
    dirty: git(["status", "--porcelain", "--untracked-files=no"], ".") !== "",
    exeBuilt: exe.mtime.toISOString(),
    os: ps("$o = Get-CimInstance Win32_OperatingSystem; \"$($o.Caption) $($o.Version) build $($o.BuildNumber)\""),
    arch: os.arch(),
    cpu: `${os.cpus()[0].model.trim()} × ${os.cpus().length}`,
    memoryGiB: Math.round(os.totalmem() / 2 ** 30),
    disk: ps(`$d = Get-Partition -DriveLetter ${drive} | Get-Disk; $p = Get-PhysicalDisk | Where-Object DeviceId -eq $d.Number; "$($d.FriendlyName), $($p.MediaType), $($d.BusType)"`),
    git: git(["--version"], "."),
    node: process.version,
    config: { runs: RUNS, warmup: WARMUP, cold: COLD, sets: SETS, only: ONLY },
    conditions: {
      cold: "a new process for every sample; the OS file cache is not flushed",
      warm: "the same action repeated in one session, after two unrecorded runs",
    },
  };
}

// --- main ------------------------------------------------------------------------------

async function main() {
  await mkdir(resolve(OUT, ".."), { recursive: true });
  await writeFile(LOG, "");
  const env = await environment();
  await note(`bench ${LABEL}: ${env.commit}${env.dirty ? "+" : ""}, ${env.cpu}, ${env.disk}`);

  const started = Date.now();
  if (!args.has("no-app")) await appPass();
  for (const set of SETS) {
    for (const [pass, run] of [["warm", warmPass], ["cold", coldPass]]) {
      if (args.has(`no-${pass}`)) continue;
      await note(`${set}: ${pass}`);
      // Every pass starts from the generated state: a failed reset must not skew the next.
      if (!args.has("keep-repos")) await rebuild(set === "network" ? ["network"] : [set]);
      await run(set).catch((error) => note(`  !! ${set} ${pass} pass stopped: ${error.message}`));
    }
  }

  const rows = [...results.values()].map((r) => ({
    id: r.id,
    group: r.group,
    title: r.title,
    set: r.set,
    condition: r.condition,
    ...summarise(r.samples.length ? r.samples : [{ total: NaN }]),
    commands: commands(r.samples),
    samples: r.samples.map((s) => ({ total: round(s.total), ipc: round(s.ipc ?? 0), front: round(s.front ?? 0) })),
    errors: r.errors,
  }));
  rows.sort((a, b) => (b.median ?? -1) - (a.median ?? -1));
  env.minutes = round((Date.now() - started) / 60000);
  await writeFile(OUT, JSON.stringify({ env, results: rows }, null, 2));

  await note(`\n${"median".padStart(9)} ${"p95".padStart(9)}  spread  scenario`);
  for (const r of rows) {
    const flag = r.spread > 0.2 ? " ⚠ noisy" : "";
    await note(`${String(r.median).padStart(9)} ${String(r.p95).padStart(9)}  ${String(r.spread).padStart(6)}  ${r.id} · ${r.set} · ${r.condition}${r.errors.length ? ` (${r.errors.length} errors)` : ""}${flag}`);
  }
  await note(`\nwritten ${OUT} in ${env.minutes} min`);

  if (args.has("check")) {
    const over = [];
    for (const budget of BUDGETS) {
      const row = rows.find((r) => r.id === budget.id && r.set === budget.set && r.condition === budget.condition);
      if (row && row.median > budget.median) over.push(`${budget.id} · ${budget.set} · ${budget.condition}: ${row.median} ms > ${budget.median} ms`);
    }
    if (over.length) {
      await note(`\nover budget:\n  ${over.join("\n  ")}`);
      process.exit(1);
    }
    await note("all budgets met");
  }
}

await main();
