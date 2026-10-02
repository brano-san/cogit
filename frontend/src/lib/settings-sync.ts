import { events } from "$lib/ipc/bindings";

/** A page-to-page event of `events`, narrowed so a test can stand in for it. */
export type Listen<T> = (handler: (event: { payload: T }) => void) => Promise<() => void>;
export type Emit<T> = (payload: T) => Promise<void>;

/** This window's mark on what it announces, so it does not read its own writes again. */
const SELF = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;

/** Every window reads the settings file on opening and keeps what it read; this is how one
    that wrote it tells the others to read it again. */
export function announceSettings(emit: Emit<string> = events.cogitSettingsChanged.emit): void {
  void emit(SELF).catch(() => {});
}

/** For a child window: reads the settings again whenever another window wrote them, so the
    theme and the date format follow Preferences without reopening it (F-335). */
export function followSettings(
  reread: () => void,
  listen: Listen<string> = events.cogitSettingsChanged.listen,
): () => void {
  return listenUntilUndone(listen, (mark) => {
    if (mark !== SELF) reread();
  });
}

/** Subscribes and returns the undo at once, for an `$effect`: the undo also holds when it
    comes before the listener is in place. */
export function listenUntilUndone<T>(listen: Listen<T>, handler: (payload: T) => void): () => void {
  let stopped = false;
  let stop: (() => void) | null = null;
  void listen((event) => {
    if (!stopped) handler(event.payload);
  })
    .then((unlisten) => {
      if (stopped) unlisten();
      else stop = unlisten;
    })
    .catch(() => {});
  return () => {
    stopped = true;
    stop?.();
  };
}
