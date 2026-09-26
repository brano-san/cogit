// The release benchmark; methods and flags in doc/15-benchmark.md.
//
//   npm run bench -- [--label baseline] [--sets small,dirty] [--only repo.open,diff.big]
//                    [--runs 10] [--warmup 2] [--cold 10] [--no-cold] [--no-warm] [--no-app]
//                    [--check] [--hidden] [--cores 16-31] [--ab old,new]

import { spawn, spawnSync } from "node:child_process";
import { appendFile, mkdir, readFile, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import { join, resolve } from "node:path";
import { EXE, dropScript, launch, resetProfile, runHidden, stop } from "./app.mjs";
import { KEYS, sleep } from "./cdp.mjs";
import { SCENARIOS } from "./scenarios.mjs";
import { BUDGETS } from "./budgets.mjs";
import { overBudget } from "./check.mjs";
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

if (args.has("hidden")) runHidden(true);

// `--cores 16-31`: this process and everything it starts (git, the app, WebView2) stay on
// those cores, off the ones the person at the machine is using.
if (args.has("cores")) {
  const [from, to] = String(args.get("cores")).split("-").map(Number);
  let mask = 0n;
  for (let core = from; core <= (to ?? from); core += 1) mask |= 1n << BigInt(core);
  spawnSync("powershell", ["-NoProfile", "-Command", `(Get-Process -Id ${process.pid}).ProcessorAffinity = [IntPtr]${mask}`]);
}

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
  /** On top of what the remote has now: the scenarios before this one put commits there,
      and a push of a branch behind its remote is refused (fetch first) — a refusal is not
      what net.push measures. `--hard` also drops a local commit an earlier failed round
      left behind; `--keep` refused once the pull scenarios had left a file stat-dirty. */
  async localCommit() {
    git(["fetch", "-q", "origin"], this.dir);
    git(["reset", "-q", "--hard", "origin/main"], this.dir);
    await this.touch("src/d01/file0001.txt");
    git(["commit", "-q", "-am", "local change"], this.dir);
    await this.settle();
  }
  /** The remote's main is the clone's HEAD, or the push did not happen. */
  pushed() {
    const head = git(["rev-parse", "HEAD"], this.dir);
    const remote = git(["ls-remote", "origin", "refs/heads/main"], this.dir).split(/\s+/)[0];
    if (head !== remote) throw new Error(`the push did not reach the remote: HEAD ${head}, remote main ${remote}`);
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
      if (!at) {
        const seen = await cdp.eval(`JSON.stringify({ filter: document.querySelector('input[placeholder="File Filter"]')?.value, headings: [...document.querySelectorAll(".pane.tree-rows .heading")].map((h) => h.textContent.trim().slice(0, 24)), rows: [...document.querySelectorAll(".pane button.row")].slice(0, 4).map((r) => r.textContent.replace(/\s+/g, " ").trim().slice(0, 30)), message: document.querySelector(".file-list .message")?.textContent })`).catch(() => "?");
        throw new Error(`nothing matches ${JSON.stringify(spec)}; the page shows ${seen}`);
      }
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
    const present = await this.exists({ sel: ".wrapper .row:not(.closed)", text: set, exact: ".name" });
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
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr OpenDesktop(string name, int flags, bool inherit, uint access);
  [DllImport("user32.dll")] public static extern bool EnumDesktopWindows(IntPtr desktop, Enum f, IntPtr l);
  public static int Close(uint owner) {
    int closed = 0;
    Enum visit = (h, l) => {
      uint pid; GetWindowThreadProcessId(h, out pid);
      var name = new StringBuilder(64); GetClassName(h, name, 64);
      if (pid == owner && name.ToString() == "#32768") {
        PostMessage(h, 0x0100, (IntPtr)0x1B, IntPtr.Zero);
        PostMessage(h, 0x0101, (IntPtr)0x1B, IntPtr.Zero);
        closed++;
      }
      return true;
    };
    EnumWindows(visit, IntPtr.Zero);
    // A run on the hidden desktop has its menu there, where EnumWindows does not look.
    IntPtr hidden = OpenDesktop("cogit-bench", 0, false, 0x0041);
    if (hidden != IntPtr.Zero) EnumDesktopWindows(hidden, visit, IntPtr.Zero);
    return closed;
  }
}
"@
[BenchMenu]::Close(${this.app.child.pid})`;
    // The menu can take a while to appear on a large repository; keep looking for 3 s.
    for (let attempt = 0; attempt < 20; attempt += 1) {
      await sleep(150);
      const r = spawnSync("powershell", ["-NoProfile", "-Command", script], { encoding: "utf8" });
      if (Number(r.stdout.trim()) > 0) break;
    }
    await this.cdp.eval("window.__bench.idle(200, 15000)");
  }
}

// --- statistics ------------------------------------------------------------------------

const quantile = (sorted, q) => sorted[Math.min(sorted.length - 1, Math.max(0, Math.ceil(q * sorted.length) - 1))];
const median = (values) => quantile([...values].sort((a, b) => a - b), 0.5);

/** Samples taken while the machine was busy are dropped when enough clean ones remain. */
const clean = (samples) => {
  const quiet = samples.filter((s) => !s.busy);
  return quiet.length >= Math.min(5, samples.length) ? quiet : samples;
};

function summarise(all) {
  const samples = clean(all);
  const totals = samples.map((s) => s.total).sort((a, b) => a - b);
  const ipcs = samples.map((s) => s.ipc ?? 0).sort((a, b) => a - b);
  const fronts = samples.map((s) => s.front ?? Math.max(s.total - (s.ipc ?? 0), 0)).sort((a, b) => a - b);
  const mid = quantile(totals, 0.5);
  const p95 = quantile(totals, 0.95);
  const longest = samples.map((s) => s.longest ?? 0).sort((a, b) => a - b);
  return {
    n: totals.length,
    busy: all.length - samples.length,
    median: round(mid),
    p95: round(p95),
    min: round(totals[0]),
    max: round(totals.at(-1)),
    ipcMedian: round(quantile(ipcs, 0.5)),
    frontMedian: round(quantile(fronts, 0.5)),
    longTaskMedian: round(quantile(longest, 0.5)),
    longTaskMax: round(longest.at(-1)),
    spread: mid > 0 ? round((p95 - mid) / mid, 3) : 0,
  };
}
const round = (v, digits = 1) => (v === undefined || v === null || Number.isNaN(v) ? null : Number(v.toFixed(digits)));

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

/** Busy: the page's fixed work took over 1.3 × its quiet level (doc/15-benchmark.md). */
const calibration = {
  seen: [],
  add(ms) {
    this.seen.push(ms);
    if (this.seen.length > 400) this.seen.shift();
  },
  level() {
    return this.seen.length >= 10 ? quantile([...this.seen].sort((a, b) => a - b), 0.25) : null;
  },
  busy(ms) {
    const level = this.level();
    return level !== null && ms > level * BUSY_FACTOR;
  },
};
const BUSY_FACTOR = Number(args.get("busy-factor") ?? 1.3);
const RETRIES = Number(args.get("retries") ?? 3);
const MAX_RUNS = Number(args.get("max-runs") ?? 30);

// --- results ---------------------------------------------------------------------------

let VARIANT = null;
const results = new Map();
const keyOf = (id, set, condition) => `${id}|${set}|${condition}|${VARIANT ?? ""}`;
function entry(id, set, condition, meta) {
  const key = keyOf(id, set, condition);
  if (!results.has(key)) results.set(key, { id, set, condition, variant: VARIANT, ...meta, samples: [], errors: [], retakes: 0 });
  return results.get(key);
}
function record(id, set, condition, sample, meta) {
  entry(id, set, condition, meta).samples.push(sample);
}
function fail(id, set, condition, error, meta) {
  entry(id, set, condition, meta).errors.push(String(error.message ?? error).slice(0, 300));
}
const samplesOf = (id, set, condition) => results.get(keyOf(id, set, condition))?.samples ?? [];

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

async function calibrate(ctx) {
  const ms = await ctx.cdp.eval("window.__bench.calibrate()");
  return ms;
}

/** A warm sample taken on a busy machine is retaken; a cold one cannot be and is only marked. */
async function attempt(scenario, ctx, condition, keep) {
  const meta = { group: scenario.group, title: scenario.title };
  if (args.has("trace")) await note(`  > ${scenario.id} (${ctx.set}, ${condition}, #${ctx.iteration})`);
  const retries = condition === "warm" ? RETRIES : 0;
  try {
    for (let tries = 0; ; tries += 1) {
      await scenario.prep?.(ctx);
      const before = await calibrate(ctx);
      const r = await scenario.measure(ctx);
      const after = await calibrate(ctx);
      await scenario.reset?.(ctx);
      const busy = calibration.busy(before) || calibration.busy(after);
      calibration.add(before);
      calibration.add(after);
      const named = r && typeof r.total === "number" ? { [scenario.id]: r } : r;
      for (const [id, sample] of Object.entries(named)) {
        if (sample.timedOut) throw new Error(`${id} timed out`);
        // A command that answered with an error is timed as fast as one that worked.
        const refused = (scenario.mustSucceed ?? []).filter((cmd) => sample.failed?.includes(cmd));
        if (refused.length > 0) throw new Error(`${id}: ${refused.join(", ")} failed`);
      }
      await scenario.verify?.(ctx);
      if (busy && tries < retries) {
        if (keep) entry(scenario.id, ctx.set, condition, meta).retakes += 1;
        continue;
      }
      if (keep) {
        for (const [id, sample] of Object.entries(named)) {
          record(id, ctx.set, condition, { ...sample, calib: round(Math.max(before, after), 2), busy }, id === scenario.id ? meta : { group: scenario.group, title: id });
        }
      }
      return;
    }
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

/** A fresh process with the repository open, and a first read of what the page runs at. */
async function session(set, exe) {
  await resetProfile();
  // The first start of a freshly built exe waits for the antivirus scan: one more try.
  const app = await launch(exe).catch(async (error) => {
    await note(`  ! launch failed, retrying: ${error.message}`);
    await resetProfile();
    return launch(exe);
  });
  const ctx = new Context(app, set);
  await limited(openRepo(ctx), 120_000, "open");
  for (let i = 0; i < 12; i += 1) calibration.add(await calibrate(ctx));
  return ctx;
}

const errorsIn = (set, condition) =>
  [...results.values()].filter((r) => r.set === set && r.condition === condition).reduce((n, r) => n + r.errors.length, 0);

/** Every scenario once per fresh process; a sample taken while the machine was busy costs
    one more launch, up to twice the planned count. */
async function coldPass(set, exe) {
  const list = applicable(set);
  if (list.length === 0) return;
  let before = errorsIn(set, "cold");
  const quietEnough = () =>
    list.every((s) => samplesOf(s.id, set, "cold").filter((x) => !x.busy).length >= COLD);
  for (let run = 0; run < WARMUP + 2 * COLD; run += 1) {
    if (run >= WARMUP + COLD && quietEnough()) break;
    // The repository is rebuilt only when the run before left it in doubt.
    if (!args.has("keep-repos") && run > 0 && errorsIn(set, "cold") > before) await rebuild([set]);
    before = errorsIn(set, "cold");
    await resetProfile();
    let app;
    try {
      app = await launch(exe);
    } catch (error) {
      await note(`  ! launch failed (cold ${set} #${run}): ${error.message}`);
      continue;
    }
    const ctx = new Context(app, set);
    ctx.iteration = run;
    for (let i = 0; i < 6; i += 1) calibration.add(await calibrate(ctx).catch(() => 0));
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
    await note(`  cold ${set} ${run + 1}`);
  }
}

/** Rounds, not runs: every scenario once per round, so a burst of someone else's load is
    spread over all of them. Then more runs for any still noisier than 20 %, up to MAX_RUNS. */
async function warmPass(set, exe) {
  const list = applicable(set);
  if (list.length === 0) return;
  let ctx = await session(set, exe);
  const recover = async (i) => {
    await stop(ctx.app, { gracefulMs: 2000 }).catch(() => {});
    ctx = await session(set, exe);
    ctx.iteration = i;
  };
  const planned = (s) => WARMUP + (s.runs ?? RUNS);
  const rounds = Math.max(...list.map(planned));
  for (let i = 0; i < rounds; i += 1) {
    for (const scenario of list) {
      if (i >= planned(scenario)) continue;
      ctx.iteration = i;
      if (!(await once(scenario, ctx, "warm", i >= WARMUP))) await recover(i);
    }
    if (args.has("trace")) await note(`  round ${i + 1}/${rounds}`);
  }
  for (const scenario of list) {
    const cap = scenario.runs ? scenario.runs * 2 : MAX_RUNS;
    for (let i = planned(scenario); ; i += 1) {
      const samples = samplesOf(scenario.id, set, "warm");
      const s = summarise(samples.length ? samples : [{ total: NaN }]);
      if (!samples.length || s.spread <= 0.2 || samples.length >= cap) break;
      ctx.iteration = i;
      if (!(await once(scenario, ctx, "warm", true))) await recover(i);
    }
    const s = summarise(samplesOf(scenario.id, set, "warm").length ? samplesOf(scenario.id, set, "warm") : [{ total: NaN }]);
    await note(`  warm ${set} ${scenario.id}${VARIANT ? ` [${VARIANT}]` : ""}: ${s.median} ms (n ${s.n}, busy ${s.busy}, spread ${s.spread})`);
  }
  await exitApp(ctx.app);
}

/** Launch to first paint, launch to ready (a repository restored), and exit. */
async function appPass(exe) {
  if (!wanted("app")) return;
  for (let run = 0; run < WARMUP + COLD; run += 1) {
    await appOnce(exe, run >= WARMUP);
    await note(`  app ${run + 1}/${WARMUP + COLD}`);
  }
}

async function appOnce(exe, keep) {
  const medium = join(REPOS, "medium").replaceAll("\\", "/");
  const meta = (title) => ({ group: "Приложение", title });
  {
    await resetProfile();
    const app = await launch(exe);
    const paint = await app.cdp.eval(`(async () => {
      for (let i = 0; i < 600; i += 1) {
        const e = performance.getEntriesByName("first-contentful-paint")[0];
        if (e) return performance.timeOrigin + e.startTime;
        await new Promise((ok) => setTimeout(ok, 25));
      }
      return null;
    })()`);
    if (keep && paint) record("app.first-paint", "empty", "cold", { total: paint - app.spawnedAt }, meta("запуск до первой отрисовки"));
    // Where the launch goes: the process and WebView2 before the page, then the page itself.
    const phases = await app.cdp.eval(`(() => {
      const n = performance.getEntriesByType("navigation")[0];
      return { origin: performance.timeOrigin, loaded: n ? n.domContentLoadedEventEnd : null };
    })()`);
    if (keep) {
      record("app.navigation-start", "empty", "cold", { total: phases.origin - app.spawnedAt }, meta("запуск: процесс и WebView2 до начала страницы"));
      if (phases.loaded) record("app.dom-loaded", "empty", "cold", { total: phases.origin + phases.loaded - app.spawnedAt }, meta("запуск: до DOMContentLoaded"));
    }
    await app.cdp.eval(`window.__bench.idle(300, 20000)`);
    // The app writes its own session on the way out, so the repository goes in the way a
    // user puts it there: opened, then remembered.
    await app.cdp.eval(`window.__bench.run(() => ${dropScript(medium)}, 300, 60000)`);
    await exitApp(app);

    const restored = await launch(exe);
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

// --- A/B -------------------------------------------------------------------------------

const EXES = resolve("target/bench/exes");
const exeOf = (name) => join(EXES, `${name}.exe`);

/** Two builds alternated round by round (AB BA AB …) on the same machine in the same hour:
    whatever else the machine is doing lands on both. Per round: a fresh process, one
    unrecorded run, then `--per-round` recorded runs of every scenario. */
async function abPass(set, names) {
  const list = applicable(set);
  if (list.length === 0) return;
  const rounds = Number(args.get("rounds") ?? 6);
  const perRound = Number(args.get("per-round") ?? 2);
  for (let r = 0; r < rounds; r += 1) {
    const order = r % 2 === 0 ? names : [...names].reverse();
    for (const name of order) {
      VARIANT = name;
      let ctx;
      try {
        ctx = await session(set, exeOf(name));
      } catch (error) {
        await note(`  ! ${name} session failed: ${error.message}`);
        continue;
      }
      for (const scenario of list) {
        for (let i = 0; i <= perRound; i += 1) {
          ctx.iteration = r * (perRound + 1) + i;
          if (!(await once(scenario, ctx, "warm", i > 0))) {
            await stop(ctx.app, { gracefulMs: 2000 }).catch(() => {});
            ctx = await session(set, exeOf(name));
            break;
          }
        }
      }
      await exitApp(ctx.app);
    }
    await note(`  ab ${set} round ${r + 1}/${rounds}`);
  }
  VARIANT = null;
}

/** 95 % interval of median(B) / median(A) by resampling both. */
function ratioInterval(a, b, draws = 2000) {
  // mulberry32: an LCG's low bits cycle too fast for small samples.
  let seed = 12345;
  const random = () => {
    seed = (seed + 0x6d2b79f5) | 0;
    let x = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    x = (x + Math.imul(x ^ (x >>> 7), 61 | x)) ^ x;
    return ((x ^ (x >>> 14)) >>> 0) / 4294967296;
  };
  const pick = (xs) => xs[Math.floor(random() * xs.length)];
  const ratios = [];
  for (let d = 0; d < draws; d += 1) {
    const ra = median(a.map(() => pick(a)));
    const rb = median(b.map(() => pick(b)));
    ratios.push(rb / ra);
  }
  ratios.sort((x, y) => x - y);
  return [quantile(ratios, 0.025), quantile(ratios, 0.975)];
}

function abTable(names) {
  const [a, b] = names;
  const lines = [];
  const pairs = new Map();
  for (const r of results.values()) {
    if (!r.variant) continue;
    const key = `${r.id}|${r.set}`;
    if (!pairs.has(key)) pairs.set(key, {});
    pairs.get(key)[r.variant] = r;
  }
  for (const [key, pair] of pairs) {
    if (!pair[a] || !pair[b]) continue;
    const xa = clean(pair[a].samples).map((s) => s.total);
    const xb = clean(pair[b].samples).map((s) => s.total);
    if (!xa.length || !xb.length) continue;
    const ma = median(xa);
    const mb = median(xb);
    const [lo, hi] = ratioInterval(xa, xb);
    const verdict = hi < 0.97 ? "faster" : lo > 1.03 ? "SLOWER" : "same";
    lines.push({ key, ma: round(ma), mb: round(mb), ratio: round(mb / ma, 3), lo: round(lo, 3), hi: round(hi, 3), verdict, na: xa.length, nb: xb.length });
  }
  return lines.sort((x, y) => y.ma - x.ma);
}

// --- main ------------------------------------------------------------------------------

function rowsOf() {
  const rows = [...results.values()].map((r) => {
    const kept = clean(r.samples);
    return {
      id: r.id,
      group: r.group,
      title: r.title,
      set: r.set,
      condition: r.condition,
      variant: r.variant ?? undefined,
      ...summarise(r.samples.length ? r.samples : [{ total: NaN }]),
      retakes: r.retakes,
      calibMedian: kept.length ? round(median(kept.map((s) => s.calib ?? 0)), 2) : null,
      commands: commands(kept),
      samples: r.samples.map((s) => ({ total: round(s.total), ipc: round(s.ipc ?? 0), front: round(s.front ?? 0), longest: round(s.longest ?? 0), calib: s.calib, busy: s.busy || undefined })),
      errors: r.errors,
    };
  });
  return rows.sort((a, b) => (b.median ?? -1) - (a.median ?? -1));
}

/** Written after every pass: an interrupted run keeps what it measured. */
async function save(env, started, extra = {}) {
  env.minutes = round((Date.now() - started) / 60000);
  env.calibrationLevel = round(calibration.level(), 2);
  await writeFile(OUT, JSON.stringify({ env, ...extra, results: rowsOf() }, null, 2));
}

async function main() {
  await mkdir(resolve(OUT, ".."), { recursive: true });
  await writeFile(LOG, "");
  const env = await environment();
  await note(`bench ${LABEL}: ${env.commit}${env.dirty ? "+" : ""}, ${env.cpu}, ${env.disk}`);
  const started = Date.now();

  if (args.has("ab")) {
    const names = String(args.get("ab")).split(",");
    env.ab = names;
    if (wanted("app") && !args.has("no-app")) {
      const rounds = Number(args.get("rounds") ?? 6);
      for (let r = 0; r < rounds * 2; r += 1) {
        for (const name of r % 2 === 0 ? names : [...names].reverse()) {
          VARIANT = name;
          await appOnce(exeOf(name), r > 0).catch((error) => note(`  ! app ${name}: ${error.message}`));
        }
        await note(`  ab app round ${r + 1}/${rounds * 2}`);
      }
      VARIANT = null;
      await save(env, started, { ab: abTable(names) });
    }
    for (const set of SETS) {
      await note(`${set}: A/B ${names.join(" vs ")}`);
      if (!args.has("keep-repos")) await rebuild([set]);
      await abPass(set, names).catch((error) => note(`  !! ${set} A/B stopped: ${error.message}`));
      await save(env, started, { ab: abTable(names) });
    }
    const table = abTable(names);
    await save(env, started, { ab: table });
    await note(`\n${names[0].padStart(10)} ${names[1].padStart(10)}   ratio   95% interval    verdict  scenario`);
    for (const l of table) {
      await note(`${String(l.ma).padStart(10)} ${String(l.mb).padStart(10)}  ${String(l.ratio).padStart(6)}  [${l.lo}, ${l.hi}]  ${l.verdict.padEnd(7)}  ${l.key}`);
    }
    await note(`\nwritten ${OUT} in ${env.minutes} min`);
    return;
  }

  const exe = args.has("exe") ? exeOf(String(args.get("exe"))) : EXE;
  env.exe = exe;
  if (!args.has("no-app")) {
    await appPass(exe);
    await save(env, started);
  }
  for (const set of SETS) {
    for (const [pass, run] of [["warm", warmPass], ["cold", coldPass]]) {
      if (args.has(`no-${pass}`)) continue;
      await note(`${set}: ${pass}`);
      // Every pass starts from the generated state: a failed reset must not skew the next.
      if (!args.has("keep-repos")) await rebuild(set === "network" ? ["network"] : [set]);
      await run(set, exe).catch((error) => note(`  !! ${set} ${pass} pass stopped: ${error.message}`));
      await save(env, started);
    }
  }

  const rows = rowsOf();
  await save(env, started);
  await note(`\n${"median".padStart(9)} ${"p95".padStart(9)}  spread  busy  long  scenario`);
  for (const r of rows) {
    const flag = r.spread > 0.2 ? " ⚠ noisy" : "";
    await note(`${String(r.median).padStart(9)} ${String(r.p95).padStart(9)}  ${String(r.spread).padStart(6)}  ${String(r.busy).padStart(4)}  ${String(r.longTaskMax ?? 0).padStart(4)}  ${r.id} · ${r.set} · ${r.condition}${r.errors.length ? ` (${r.errors.length} errors)` : ""}${flag}`);
  }
  await note(`\nwritten ${OUT} in ${env.minutes} min`);

  if (args.has("check")) {
    // App budgets come from the app pass, the rest from the warm pass of their set.
    const expected = (budget) =>
      wanted(budget.id) &&
      (budget.id.startsWith("app.")
        ? wanted("app") && !args.has("no-app")
        : SETS.includes(budget.set) && !args.has(`no-${budget.condition}`));
    const over = overBudget(rows, BUDGETS, expected);
    if (over.length) {
      await note(`\nover budget:\n  ${over.join("\n  ")}`);
      process.exit(1);
    }
    await note("all budgets met");
  }
}

await main();
