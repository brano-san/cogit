export const STALL_MS = 100;
export const SAMPLE_MS = 1000;
export const QUIET_MS = 10_000;

export function stallReport(cause: string, gaps: number[]): string | null {
  const worst = Math.max(...gaps);
  if (gaps.length === 0 || worst < STALL_MS) return null;
  const median = [...gaps].sort((a, b) => a - b)[Math.floor(gaps.length / 2)] ?? 0;
  const slow = gaps.filter((gap) => gap >= 50).length;
  return `after ${cause}: ${gaps.length} frames in ${Math.round(gaps.reduce((a, b) => a + b, 0))}ms, median ${Math.round(median)}ms, worst ${Math.round(worst)}ms, ${slow} over 50ms`;
}

export function watchFrameStalls(target: Window, log: (message: string) => void): () => void {
  let sampling = false;
  let lastLog = -Infinity;

  const sample = (event: Event) => {
    if (sampling || performance.now() - lastLog < QUIET_MS) return;
    sampling = true;
    const started = performance.now();
    const gaps: number[] = [];
    let previous = started;
    const frame = (now: number) => {
      gaps.push(now - previous);
      previous = now;
      if (now - started < SAMPLE_MS) {
        target.requestAnimationFrame(frame);
        return;
      }
      sampling = false;
      const report = stallReport(event.type, gaps);
      if (report) {
        lastLog = now;
        log(report);
      }
    };
    target.requestAnimationFrame(frame);
  };

  const kinds = ["pointerdown", "wheel", "keydown"] as const;
  for (const kind of kinds) target.addEventListener(kind, sample, { capture: true, passive: true });
  return () => {
    for (const kind of kinds) target.removeEventListener(kind, sample, { capture: true });
  };
}
