/** The end of the task: effects of one update land together. Any later and the benchmark,
    which ends an action at its last IPC call, would count the wait (R-322). */
export const MENU_STATE_DELAY_MS = 0;

type Send = (disabled: string[], checked: string[]) => Promise<unknown>;

/** `set_menu_state` walks every item of the bar on the main thread, and the effect that
    feeds it re-ran six times and more per action, mostly with the same state. This sends
    the last state of a burst once, and nothing when the bar already shows it. */
export function menuStatePusher(send: Send, delayMs = MENU_STATE_DELAY_MS) {
  let sent: string | null = null;
  let latest: { disabled: string[]; checked: string[] } | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;

  function flush() {
    timer = null;
    if (!latest) return;
    const { disabled, checked } = latest;
    latest = null;
    const key = JSON.stringify([[...disabled].sort(), [...checked].sort()]);
    if (key === sent) return;
    sent = key;
    void send(disabled, checked).catch(() => {
      if (sent === key) sent = null;
    });
  }

  /** `resend`: the bar may not show what was sent — it was rebuilt with every tick
      cleared, or muda flipped a tick on a click. */
  return (disabled: string[], checked: string[], options: { resend?: boolean } = {}) => {
    if (options.resend) sent = null;
    latest = { disabled, checked };
    timer ??= setTimeout(flush, delayMs);
  };
}
