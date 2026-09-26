import { emit as tauriEmit, listen as tauriListen } from "@tauri-apps/api/event";

/** Every window reads the settings file on opening and keeps what it read; this is how one
    that wrote it tells the others to read it again. */
export const SETTINGS_CHANGED = "cogit://settings-changed";

export type Listen = (name: string, handler: (event: { payload: unknown }) => void) => Promise<() => void>;
export type Emit = (name: string, payload: unknown) => Promise<void>;

/** This window's mark on what it announces, so it does not read its own writes again. */
const SELF = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;

export function announceSettings(emit: Emit = tauriEmit): void {
  void emit(SETTINGS_CHANGED, SELF).catch(() => {});
}

/** For a child window: reads the settings again whenever another window wrote them, so the
    theme and the date format follow Preferences without reopening it (F-335). Returns the
    undo, which also holds if it comes before the listener is in place. */
export function followSettings(reread: () => void, listen: Listen = tauriListen): () => void {
  let stopped = false;
  let stop: (() => void) | null = null;
  void listen(SETTINGS_CHANGED, (event) => {
    if (!stopped && event.payload !== SELF) reread();
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
