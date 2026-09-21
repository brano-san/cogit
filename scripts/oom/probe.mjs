// Drives the running app over the DevTools Protocol and records what the renderer holds.
//
// Two independent series come out of a run, and they check each other:
//   - this file's CSV, from `Performance.getMetrics` (the engine's own counters);
//   - the app's `kind=mem` lines in cogit.log, from the in-page probe.
//
// Needs the debug overlay, which is what opens the port:
//   npm run tauri dev -- --config src-tauri/tauri.debug.conf.json
//
//   node scripts/oom/probe.mjs --repo <path> --out <dir> [--port 9222] [--selects 500]

import { mkdir, writeFile, appendFile } from "node:fs/promises";
import { createWriteStream } from "node:fs";
import { resolve } from "node:path";

const args = new Map();
for (let i = 2; i < process.argv.length; i += 2) {
  args.set(process.argv[i].replace(/^--/, ""), process.argv[i + 1]);
}

const PORT = Number(args.get("port") ?? 9222);
const REPO = args.get("repo");
const OUT = resolve(args.get("out") ?? "oom-run");
const SELECTS = Number(args.get("selects") ?? 500);

if (!REPO) {
  console.error("usage: node scripts/oom/probe.mjs --repo <path> --out <dir>");
  process.exit(2);
}

const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));

/** Graph rows carry no test id; the list role is the stable hook. */
const ROWS = JSON.stringify(`[role="listitem"]`);
const ROW_COUNT = `document.querySelectorAll(${ROWS}).length`;

/** The window takes a moment to exist; a refused connection here is normal, not fatal. */
async function pageTarget(timeoutMs = 90_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${PORT}/json`);
      const targets = await response.json();
      const page = targets.find((t) => t.type === "page" && t.webSocketDebuggerUrl);
      if (page) return page;
    } catch {
      // Not listening yet.
    }
    await sleep(500);
  }
  throw new Error(`no page target on port ${PORT} after ${timeoutMs}ms`);
}

function client(url) {
  const socket = new WebSocket(url);
  const pending = new Map();
  const listeners = new Map();
  let nextId = 1;

  const ready = new Promise((ok, fail) => {
    socket.addEventListener("open", ok, { once: true });
    socket.addEventListener("error", () => fail(new Error("cdp socket failed")), { once: true });
  });

  socket.addEventListener("message", (event) => {
    const frame = JSON.parse(event.data);
    if (frame.id !== undefined) {
      const slot = pending.get(frame.id);
      if (!slot) return;
      pending.delete(frame.id);
      frame.error ? slot.fail(new Error(JSON.stringify(frame.error))) : slot.ok(frame.result);
      return;
    }
    for (const handler of listeners.get(frame.method) ?? []) handler(frame.params);
  });

  return {
    ready,
    send(method, params = {}) {
      const id = nextId++;
      socket.send(JSON.stringify({ id, method, params }));
      return new Promise((ok, fail) => pending.set(id, { ok, fail }));
    },
    /** Returns the function that detaches it: a snapshot handler that outlives its
        snapshot keeps every chunk of it alive, which in this script is the bug itself. */
    on(method, handler) {
      listeners.set(method, [...(listeners.get(method) ?? []), handler]);
      return () => listeners.set(method, (listeners.get(method) ?? []).filter((h) => h !== handler));
    },
    close: () => socket.close(),
  };
}

/** `Performance.getMetrics` is a flat name/value list; these are the ones that matter. */
const WANTED = ["JSHeapUsedSize", "JSHeapTotalSize", "Nodes", "JSEventListeners", "Documents"];

async function metrics(cdp) {
  const { metrics: all } = await cdp.send("Performance.getMetrics");
  const found = Object.fromEntries(all.map((m) => [m.name, m.value]));
  return Object.fromEntries(WANTED.map((name) => [name, found[name] ?? 0]));
}

async function evaluate(cdp, expression) {
  const result = await cdp.send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (result.exceptionDetails) {
    throw new Error(result.exceptionDetails.exception?.description ?? "evaluate failed");
  }
  return result.result.value;
}

/** A snapshot without a preceding GC counts garbage that was already on its way out. */
async function snapshot(cdp, file) {
  await cdp.send("HeapProfiler.collectGarbage");

  // Streamed, not joined: a snapshot of a large graph runs to hundreds of megabytes, and
  // building one string of it would put this script in the same trouble it is measuring.
  const out = createWriteStream(file);
  let bytes = 0;
  const detach = cdp.on("HeapProfiler.addHeapSnapshotChunk", (params) => {
    bytes += params.chunk.length;
    out.write(params.chunk);
  });

  await cdp.send("HeapProfiler.takeHeapSnapshot", { reportProgress: false });
  detach();
  await new Promise((ok) => out.end(ok));
  return bytes;
}

async function main() {
  await mkdir(OUT, { recursive: true });
  const csv = resolve(OUT, "metrics.csv");
  await writeFile(csv, `ms,phase,${WANTED.join(",")}\n`);

  const target = await pageTarget();
  console.log(`attached to ${target.url}`);
  const cdp = client(target.webSocketDebuggerUrl);
  await cdp.ready;

  await cdp.send("Runtime.enable");
  await cdp.send("Page.enable");
  await cdp.send("Performance.enable");
  await cdp.send("HeapProfiler.enable");

  const started = Date.now();
  let phase = "baseline";
  const record = async () => {
    const row = await metrics(cdp);
    await appendFile(
      csv,
      `${Date.now() - started},${phase},${WANTED.map((n) => row[n]).join(",")}\n`,
    );
    return row;
  };

  // Sampled throughout, denser than the app's own ten-second probe.
  const ticking = setInterval(() => void record().catch(() => {}), 2000);

  const base = await record();
  console.log("baseline", base);
  await snapshot(cdp, resolve(OUT, "1-baseline.heapsnapshot"));

  // The session is the product's own way back into a repository, so this opens it the
  // way the user does rather than through a back door.
  phase = "open";
  const escaped = JSON.stringify(REPO);
  await evaluate(
    cdp,
    `localStorage.setItem('cogit.session.v1', JSON.stringify({
       repositories: [${escaped}], active: ${escaped}, selected: {}, recent: [${escaped}]
     })), 'ok'`,
  );
  await cdp.send("Page.reload");
  await sleep(2000);

  // The graph streams in chunks; it is done when the row count stops moving.
  let previous = -1;
  let settled = 0;
  for (let i = 0; i < 300 && settled < 3; i += 1) {
    await sleep(1000);
    const rows = await evaluate(cdp, ROW_COUNT).catch(() => 0);
    const now = await metrics(cdp);
    settled = now.Nodes === previous ? settled + 1 : 0;
    previous = now.Nodes;
    if (i % 5 === 0) console.log(`  loading… nodes=${now.Nodes} heap=${mib(now.JSHeapUsedSize)} rows=${rows}`);
  }

  phase = "loaded";
  const loaded = await record();
  console.log("loaded", loaded);
  await snapshot(cdp, resolve(OUT, "2-loaded.heapsnapshot"));

  // P1 item 13: selecting commits down the list, which is the action a user repeats most.
  phase = "selecting";
  for (let i = 0; i < SELECTS; i += 1) {
    await evaluate(
      cdp,
      `(() => {
         const rows = document.querySelectorAll(${ROWS});
         if (rows.length === 0) return 0;
         rows[${i} % rows.length].dispatchEvent(new MouseEvent('click', { bubbles: true }));
         return rows.length;
       })()`,
    ).catch(() => {});
    if (i % 50 === 0) await sleep(200);
  }
  await sleep(3000);

  phase = "after-selects";
  const after = await record();
  console.log("after selects", after);
  await snapshot(cdp, resolve(OUT, "3-after-selects.heapsnapshot"));

  clearInterval(ticking);

  const grew = after.JSHeapUsedSize - base.JSHeapUsedSize;
  console.log(`\nheap ${mib(base.JSHeapUsedSize)} → ${mib(after.JSHeapUsedSize)} (+${mib(grew)})`);
  console.log(`nodes ${base.Nodes} → ${after.Nodes}, listeners ${base.JSEventListeners} → ${after.JSEventListeners}`);
  console.log(`csv: ${csv}`);
  cdp.close();
}

function mib(bytes) {
  return `${(bytes / 1024 / 1024).toFixed(1)} MiB`;
}

await main();
