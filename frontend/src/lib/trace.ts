import { commands, type WebviewLogLine } from "$lib/ipc/bindings";

/** One line in `cogit.log`, written from the webview, with the backend's timestamp.
    There was no way to see the frontend half of a stall: the command existed and nothing
    called it (doc/12-risks.md, R-120). */
export interface TraceLine {
  context: string;
  message: string;
  /** Milliseconds since the window started, so a stall is visible without subtracting. */
  at: number;
}

/** Lines share one IPC call per batch: an action wrote 9–12, each a call of its own.
    The batch closes at the end of the task, not later: the benchmark takes the last IPC
    call as the end of the action, so a deferred write would read as a slower action
    (doc/12-risks.md, R-321). */
export const TRACE_BATCH_LINES = 32;
export const TRACE_BATCH_MS = 0;

type Level = "info" | "error";

let origin = 0;
let pending: WebviewLogLine[] = [];
let timer: ReturnType<typeof setTimeout> | null = null;
let page: EventTarget | undefined;

/** `target` is where `pagehide` fires; the window, unless a test hands its own. */
export function startTracing(now = performance.now(), target: EventTarget | undefined = globalThis.window): void {
  origin = now;
  page?.removeEventListener("pagehide", flushTrace);
  page = target;
  page?.addEventListener("pagehide", flushTrace);
}

/** `context` groups the lines of one story, e.g. `open:C:/repos/dtv_device`. */
export function line(context: string, message: string, now = performance.now()): TraceLine {
  return { context, message, at: Math.round(now - origin) };
}

export function format(entry: TraceLine): string {
  return `+${entry.at}ms ${entry.message}`;
}

/** Fire and forget. An error goes out at once, with the lines queued before it: the
    renderer may not live to the timer. */
export function trace(context: string, message: string, level: Level = "info"): void {
  const entry = line(context, message);
  pending.push({ level, message: format(entry), context: entry.context });
  if (level === "error" || pending.length >= TRACE_BATCH_LINES) flushTrace();
  else timer ??= setTimeout(flushTrace, TRACE_BATCH_MS);
}

/** Sends what waits now; the exit path calls it before the window goes. It swallows
    everything: tracing that can break the thing it is tracing is worse than no tracing. */
export function flushTrace(): void {
  if (timer !== null) clearTimeout(timer);
  timer = null;
  if (pending.length === 0) return;
  const lines = pending;
  pending = [];
  try {
    void commands.logFromFrontend(lines)?.catch(() => {});
  } catch {
    // No bridge here — a test, or a window that has already gone.
  }
}

/** Times a step and writes one line when it ends, whichever way it ends. */
export async function timed<T>(context: string, what: string, run: () => Promise<T>): Promise<T> {
  const started = performance.now();
  try {
    const value = await run();
    trace(context, `${what} ok in ${Math.round(performance.now() - started)}ms`);
    return value;
  } catch (err) {
    trace(context, `${what} failed in ${Math.round(performance.now() - started)}ms: ${String(err)}`, "error");
    throw err;
  }
}
