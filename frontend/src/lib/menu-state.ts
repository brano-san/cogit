/** Long enough to gather one action's store updates, short enough that a menu opened
    right after it already shows the new state. */
export const MENU_STATE_DELAY_MS = 30;

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

  /** `rebuilt`: the bar was built anew with every tick cleared, so the last state sent
      is no longer on it. */
  return (disabled: string[], checked: string[], options: { rebuilt?: boolean } = {}) => {
    if (options.rebuilt) sent = null;
    latest = { disabled, checked };
    timer ??= setTimeout(flush, delayMs);
  };
}
