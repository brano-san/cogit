// Kills the renderer on purpose, to see what the app does about it.
//
// `Page.crash` is the DevTools Protocol's own way to end a render process, so this
// exercises the real `ICoreWebView2::add_ProcessFailed` path rather than a simulation.
// The app should answer with its own dialog and a `kind="render-process-exited"` line
// in the log; it must never show Edge's built-in error page.
//
//   node scripts/oom/crash-test.mjs [--port 9222]

const args = new Map();
for (let i = 2; i < process.argv.length; i += 2) {
  args.set(process.argv[i].replace(/^--/, ""), process.argv[i + 1]);
}
const PORT = Number(args.get("port") ?? 9222);

const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));

async function pageTarget(timeoutMs = 60_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const targets = await (await fetch(`http://127.0.0.1:${PORT}/json`)).json();
      const page = targets.find((t) => t.type === "page" && t.webSocketDebuggerUrl);
      if (page) return page;
    } catch {
      // Not listening yet.
    }
    await sleep(500);
  }
  throw new Error(`no page target on port ${PORT}`);
}

const target = await pageTarget();
console.log(`attached to ${target.url}`);

const socket = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((ok, fail) => {
  socket.addEventListener("open", ok, { once: true });
  socket.addEventListener("error", () => fail(new Error("cdp socket failed")), { once: true });
});

// The reply never comes: the process answering it is the one being ended.
socket.send(JSON.stringify({ id: 1, method: "Page.crash", params: {} }));
console.log("Page.crash sent; the renderer should be gone");

await sleep(3000);
socket.close();

console.log(
  "\nNow check the log for the handler's line:\n" +
    "  grep 'lost a process' %LOCALAPPDATA%\\dev.branosan.cogit-oomtest\\logs\\cogit-*.log",
);
