/** How many Tauri subscriptions are live right now.

    P1 item 5 — a `listen()` whose unlisten is never called — is invisible from the heap
    size alone: the leak is a handful of closures holding whatever they captured. Counting
    them here rather than reaching into `__TAURI_EVENT_PLUGIN_INTERNALS__` keeps the probe
    off an undocumented shape that moves between Tauri releases. */

let live = 0;

export function liveListeners(): number {
  return live;
}

/** Wraps a pending subscription so the count follows it. The returned stop is idempotent:
    Svelte can run an effect cleanup twice, and a double decrement would hide a real leak
    behind a negative number. */
export async function counted(pending: Promise<() => void>): Promise<() => void> {
  const stop = await pending;
  live += 1;
  let stopped = false;

  return () => {
    if (stopped) return;
    stopped = true;
    live -= 1;
    stop();
  };
}

/** Tests only: the counter is module state shared by every subscription. */
export function resetListenerCount(): void {
  live = 0;
}
