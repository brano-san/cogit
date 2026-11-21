import type { KeyBinding } from "$lib/ipc/bindings";
import { canonicalShortcut, nativeMacShortcut, pressedAccelerator, type KeyPress, type Keymap } from "./keymap";

/** The menu command the page runs for a key itself, or null when the window takes the key.
    On Windows the WebView2 hook claims every key of the keymap first. Elsewhere the native
    menu holds the bar's keys, so the page takes a command that is not on the bar
    (`OFF_THE_BAR`: Ctrl+K, F5, Ctrl+Shift+Enter, Ctrl+Shift+Y), and a bar key typed with a
    non-Latin layout, which GTK matches by character and misses (R-740). */
export function pageCommand(
  press: KeyPress,
  bindings: readonly KeyBinding[],
  keys: Keymap,
  platform: { windows: boolean; mac: boolean },
): string | null {
  if (platform.windows) return null;
  const pressed = pressedAccelerator(press, platform.mac);
  if (pressed === null || (platform.mac && nativeMacShortcut(pressed))) return null;
  const chord = canonicalShortcut(pressed, platform.mac);
  const binding = bindings.find((entry) => {
    const bound = keys[entry.id] ?? "";
    return bound !== "" && canonicalShortcut(bound, platform.mac) === chord;
  });
  if (!binding) return null;
  const missed = press.key.length === 1 && press.key.charCodeAt(0) >= 128;
  return !binding.onBar || missed ? binding.id : null;
}
