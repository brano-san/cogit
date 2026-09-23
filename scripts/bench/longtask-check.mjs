// node scripts/bench/longtask-check.mjs [exe] — does the webview report long tasks at all?
// Blocks the main thread for 120 ms and reads what `PerformanceObserver('longtask')` saw,
// both directly and through the bench page script. An empty answer would make every
// "longest: 0" in the results meaningless.

import { resolve } from "node:path";
import { launch, resetProfile, runHidden, stop } from "./app.mjs";

runHidden(!process.argv.includes("--shown"));
const exe = process.argv.slice(2).find((arg) => !arg.startsWith("--"));
await resetProfile();
const app = await launch(exe ? resolve(exe) : undefined);
try {
  const direct = await app.cdp.eval(
    `new Promise((done) => {
      const seen = [];
      new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) seen.push(Math.round(entry.duration));
      }).observe({ type: "longtask" });
      setTimeout(() => { const t = performance.now(); while (performance.now() - t < 120) {} }, 50);
      setTimeout(() => done(seen), 800);
    })`,
    10_000,
  );
  const bench = await app.cdp.eval(
    `window.__bench.run(async () => { const t = performance.now(); while (performance.now() - t < 120) {} }, 150, 10000)`,
    20_000,
  );
  console.log(`observer saw: ${JSON.stringify(direct)} ms; bench measure: longest ${Math.round(bench.longest ?? -1)} ms of ${Math.round(bench.total)} ms`);
} finally {
  await app.cdp.eval("window.__bench && 0", 2000).catch(() => {});
  app.child.kill();
  await stop(app, { gracefulMs: 3000 }).catch(() => {});
}
