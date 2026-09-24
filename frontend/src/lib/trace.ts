import { commands } from "$lib/ipc/bindings";

/** One line in `cogit.log`, written from the webview, with the backend's timestamp.
    There was no way to see the frontend half of a stall: the command existed and nothing
    called it (doc/12-risks.md, R-120). */
export interface TraceLine {
  context: string;
  message: string;
  /** Milliseconds since the window started, so a stall is visible without subtracting. */
  at: number;
}

let origin = 0;

export function startTracing(now = performance.now()): void {
  origin = now;
}

/** `context` groups the lines of one story, e.g. `open:C:/repos/dtv_device`. */
export function line(context: string, message: string, now = performance.now()): TraceLine {
  return { context, message, at: Math.round(now - origin) };
}

export function format(entry: TraceLine): string {
  return `+${entry.at}ms ${entry.message}`;
}

/** Fire and forget, and it swallows everything: tracing that can break the thing it is
    tracing is worse than no tracing. */
export function trace(context: string, message: string): void {
  const entry = line(context, message);
  try {
    void commands.logFromFrontend("info", format(entry), entry.context)?.catch(() => {});
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
    trace(context, `${what} failed in ${Math.round(performance.now() - started)}ms: ${String(err)}`);
    throw err;
  }
}
