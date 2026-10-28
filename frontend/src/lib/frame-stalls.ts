export const STALL_MS = 100;
export const SAMPLE_MS = 1000;
export const QUIET_MS = 2000;
export const WATCHDOG_MS = 500;
export const BLOCKED_MS = 250;

export interface Breakdown {
  delay: number;
  handlers: number;
  firstFrame: number;
}

export function stallReport(cause: string, gaps: number[], breakdown?: Breakdown, where = ""): string | null {
  const worst = Math.max(...gaps);
  if (gaps.length === 0 || worst < STALL_MS) return null;
  const median = [...gaps].sort((a, b) => a - b)[Math.floor(gaps.length / 2)] ?? 0;
  const slow = gaps.filter((gap) => gap >= 50).length;
  const total = Math.round(gaps.reduce((a, b) => a + b, 0));
  let report = `after ${cause}: ${gaps.length} frames in ${total}ms, median ${Math.round(median)}ms, worst ${Math.round(worst)}ms, ${slow} over 50ms`;
  if (breakdown) {
    report += `; input delay ${Math.round(breakdown.delay)}ms, handlers ${Math.round(breakdown.handlers)}ms, first frame ${Math.round(breakdown.firstFrame)}ms`;
  }
  if (where) report += `; on ${where}`;
  return report;
}

export function describeTarget(target: EventTarget | null): string {
  const parts: string[] = [];
  let el = target instanceof Element ? target : null;
  while (el && parts.length < 4 && el !== document.body) {
    const cls = typeof el.className === "string" ? el.className.trim().split(/\s+/).find((name) => !name.startsWith("svelte-")) : "";
    parts.push(el.id ? `${el.localName}#${el.id}` : cls ? `${el.localName}.${cls}` : el.localName);
    el = el.parentElement;
  }
  return parts.reverse().join(" > ");
}

export function watchFrameStalls(target: Window, log: (message: string) => void): () => void {
  let sampling = false;
  let lastLog = -Infinity;
  let lastInput = "none";

  const sample = (event: Event) => {
    const where = event.type === "resize" ? "window" : describeTarget(event.target);
    lastInput = `${event.type} on ${where}`;
    if (sampling || performance.now() - lastLog < QUIET_MS) return;
    if (event.type === "pointermove" && (event as PointerEvent).buttons === 0) return;
    sampling = true;
    const started = performance.now();
    const breakdown: Breakdown = { delay: Math.max(0, started - event.timeStamp), handlers: 0, firstFrame: 0 };
    setTimeout(() => {
      breakdown.handlers = performance.now() - started;
    }, 0);
    const gaps: number[] = [];
    let previous = started;
    const frame = (now: number) => {
      if (gaps.length === 0) breakdown.firstFrame = now - started;
      gaps.push(now - previous);
      previous = now;
      if (now - started < SAMPLE_MS) {
        target.requestAnimationFrame(frame);
        return;
      }
      sampling = false;
      const nodes = document.getElementsByTagName("*").length;
      const report = stallReport(event.type, gaps, breakdown, `${where} (${nodes} DOM nodes)`);
      if (report) {
        lastLog = now;
        log(report);
      }
    };
    target.requestAnimationFrame(frame);
  };

  let expected = performance.now() + WATCHDOG_MS;
  let lastBlocked = -Infinity;
  const watchdog = setInterval(() => {
    const now = performance.now();
    const late = now - expected;
    expected = now + WATCHDOG_MS;
    if (document.hidden || late < BLOCKED_MS || now - lastBlocked < QUIET_MS) return;
    lastBlocked = now;
    log(`main thread blocked ${Math.round(late)}ms; last input ${lastInput}`);
  }, WATCHDOG_MS);

  const kinds = ["pointerdown", "pointermove", "wheel", "keydown", "resize"] as const;
  for (const kind of kinds) target.addEventListener(kind, sample, { capture: true, passive: true });
  return () => {
    clearInterval(watchdog);
    for (const kind of kinds) target.removeEventListener(kind, sample, { capture: true });
  };
}
