/** Below this the number says nothing a human would act on, and scrolling would spam it. */
export const REPORT_FLOOR_MS = 120;

export interface Timer {
  stop(detail?: string): void;
}

export type Sink = (label: string, ms: number, detail: string) => void;

/** The clock and the sink are parameters so the arithmetic is testable without a backend. */
export function timer(label: string, sink: Sink, now: () => number): Timer {
  const started = now();
  let done = false;

  return {
    stop(detail = "") {
      if (done) return;
      done = true;
      const ms = Math.round(now() - started);
      if (ms < REPORT_FLOOR_MS) return;
      try {
        sink(label, ms, detail);
      } catch {
        // A lost measurement must never break the action it was measuring.
      }
    },
  };
}

/** Bound to a real sink once, at the app's edge, so this module stays free of IPC. */
export function measurer(sink: Sink): (label: string) => Timer {
  return (label) => timer(label, sink, () => performance.now());
}
