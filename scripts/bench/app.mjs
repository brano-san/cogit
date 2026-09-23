import { spawn, spawnSync } from "node:child_process";
import { mkdir, writeFile, rm } from "node:fs/promises";
import { join, resolve } from "node:path";
import { connect, pageTarget, sleep } from "./cdp.mjs";
import { PAGE } from "./page.mjs";

export const PORT = 9333;
export const IDENTIFIER = "dev.branosan.cogit-bench";
export const EXE = resolve("target/bench/release/cogit.exe");
export const CONFIG_DIR = join(process.env.APPDATA ?? "", IDENTIFIER);
export const DATA_DIR = join(process.env.LOCALAPPDATA ?? "", IDENTIFIER);

export const SETTINGS = {
  settings: {
    theme: "dark",
    dateFormat: "relative",
    avatars: "off",
    autoUpdate: false,
    confirmExit: false,
    logLevel: "info",
  },
};

/** WebView2's own processes outlive cogit.exe by a moment and keep the profile locked.
    Only ours are touched: the bench profile folder is in their command line. */
function killStrayWebviews() {
  const script = `Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object { $_.CommandLine -like '*${IDENTIFIER}*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }`;
  spawnSync("powershell", ["-NoProfile", "-Command", script]);
}

export async function resetProfile() {
  await mkdir(CONFIG_DIR, { recursive: true });
  await writeFile(join(CONFIG_DIR, "settings.json"), JSON.stringify(SETTINGS, null, 2));
  const webview = join(DATA_DIR, "EBWebView");
  try {
    await rm(webview, { recursive: true, force: true, maxRetries: 30, retryDelay: 100 });
  } catch {
    killStrayWebviews();
    await rm(webview, { recursive: true, force: true, maxRetries: 30, retryDelay: 200 });
  }
}

export async function launch() {
  const spawnedAt = Date.now();
  const child = spawn(EXE, [], { stdio: "ignore", env: { ...process.env } });
  const exited = new Promise((ok) => child.on("exit", () => ok(Date.now())));
  try {
    return await attach(child, spawnedAt, exited);
  } catch (error) {
    child.kill();
    await Promise.race([exited, sleep(5000)]);
    killStrayWebviews();
    throw error;
  }
}

async function attach(child, spawnedAt, exited) {
  const target = await pageTarget(PORT, 30_000);
  const cdp = await connect(target.webSocketDebuggerUrl);
  await cdp.send("Runtime.enable");
  await cdp.send("Page.enable");
  await cdp.send("Page.addScriptToEvaluateOnNewDocument", { source: PAGE });
  // The webview may navigate once more after the target appears: wait for the app's document.
  for (let attempt = 0; ; attempt += 1) {
    try {
      const ready = await cdp.eval(
        `document.readyState === "complete" && !!window.__TAURI_INTERNALS__ && !!document.querySelector("#app")?.firstElementChild`,
        5000,
      );
      if (ready) {
        await cdp.eval(PAGE, 5000);
        await cdp.eval("window.__bench.patch(), true", 5000);
        break;
      }
    } catch (error) {
      if (attempt > 200) throw error;
    }
    if (attempt > 400) throw new Error("the app document never finished loading");
    await sleep(25);
  }
  return { child, cdp, spawnedAt, exited };
}

export async function stop(app, { gracefulMs = 8000 } = {}) {
  const deadline = sleep(gracefulMs).then(() => null);
  const gone = await Promise.race([app.exited, deadline]);
  if (gone === null) {
    app.child.kill();
    await app.exited;
  }
  app.cdp.close();
}

/** Opens a repository the way a folder dropped on the window does (App.svelte, onDragDrop). */
export const dropScript = (path) =>
  `window.__bench.emit("tauri://drag-drop", { paths: [${JSON.stringify(path)}], position: { x: 400, y: 300 } })`;
