import type { KeyBinding } from "$lib/ipc/bindings";
import { foreign, ON_MAC, primary } from "$lib/platform";

/** Command id to accelerator. An empty string means the user removed the key. */
export type Keymap = Record<string, string>;

export function effective(bindings: readonly KeyBinding[], overrides: Keymap, onMac = ON_MAC): Keymap {
  const map: Keymap = {};
  for (const binding of bindings) {
    const override = overrides[binding.id];
    const fallback = binding.defaultAccelerator ?? "";
    const keys = onMac && (fixedMacCommand(binding.id) || (override && nativeMacShortcut(override)))
      ? fallback
      : override ?? fallback;
    map[binding.id] = onMac ? keys : keys.replace(/^Ctrl\+/, "CmdOrCtrl+");
  }
  return map;
}

/** For each bound command, the other commands claiming the same key. */
export function conflicts(
  bindings: readonly KeyBinding[],
  overrides: Keymap,
): Record<string, string[]> {
  const map = effective(bindings, overrides);
  const byKey = new Map<string, string[]>();
  for (const [id, keys] of Object.entries(map)) {
    if (keys === "") continue;
    const bucket = byKey.get(keys);
    if (bucket) bucket.push(id);
    else byKey.set(keys, [id]);
  }

  const clashes: Record<string, string[]> = {};
  for (const ids of byKey.values()) {
    if (ids.length < 2) continue;
    for (const id of ids) clashes[id] = ids.filter((other) => other !== id);
  }
  return clashes;
}

const MODIFIERS = new Set(["cmdorctrl", "commandorcontrol", "ctrl", "control", "cmd", "command", "shift", "alt", "option"]);
const FUNCTION_KEY = /^F(?:[1-9]|1\d|2[0-4])$/;
const OTHER_KEYS = new Set(["ENTER", "RETURN", "LEFT", "UP", "RIGHT", "DOWN", "TAB", "PAGEUP", "PAGEDOWN", ",", ".", "-", "="]);

/** Whether the window takes this accelerator for its command — the rule of
    `accelerators::parse` in Rust: Ctrl (⌘) or Alt with a letter, a digit, `, . - =`, Enter
    Tab, PageUp, PageDown or an arrow; an F-key with or without them. A key outside it shows in the menu and does
    nothing, because WebView2 keeps it from the menu. Both sides test
    `accelerator-cases.json`. */
export function claimable(keys: string): boolean {
  let key: string | null = null;
  let ctrlOrAlt = false;
  for (const raw of keys.split("+")) {
    const part = raw.trim();
    if (part === "") return false;
    const lower = part.toLowerCase();
    if (MODIFIERS.has(lower)) {
      if (lower !== "shift") ctrlOrAlt = true;
      continue;
    }
    if (key !== null) return false;
    key = part.toUpperCase();
  }
  if (key === null) return false;
  if (FUNCTION_KEY.test(key)) return true;
  return ctrlOrAlt && (/^[A-Z0-9]$/.test(key) || OTHER_KEYS.has(key));
}

/** A key press as the editor reads it; a `KeyboardEvent` is one. */
export interface KeyPress {
  key: string;
  code: string;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  getModifierState?: (key: string) => boolean;
}

export type Recorded = { keys: string } | { refused: string };

const MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "AltGraph", "Meta", "OS"]);

/** By where the key sits, not by what the layout types there: `Ы`, `&` or `б` is no
    accelerator the menu can read. Letters, digits and F-keys are read by pattern. */
const PLACES: Record<string, string> = {
  Comma: ",",
  Period: ".",
  Minus: "-",
  Equal: "=",
  Enter: "Enter",
  NumpadEnter: "Enter",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Tab: "Tab",
  PageUp: "PageUp",
  PageDown: "PageDown",
};

/** Names for keys the window cannot take, to say which one it was. */
const SHOWN: Record<string, string> = { " ": "Space", Escape: "Esc" };

const FOR_TYPING = "is kept for text fields and panels";

/** Taken by the window, these would stop working where the user types and in the focused
    panel (11 §11); Alt+F4 is the system's. */
const RESERVED: Record<string, string> = {
  "CmdOrCtrl+A": FOR_TYPING,
  "CmdOrCtrl+C": FOR_TYPING,
  "CmdOrCtrl+V": FOR_TYPING,
  "CmdOrCtrl+X": FOR_TYPING,
  "CmdOrCtrl+Z": FOR_TYPING,
  "CmdOrCtrl+Shift+Z": FOR_TYPING,
  "CmdOrCtrl+Y": FOR_TYPING,
  "CmdOrCtrl+F": FOR_TYPING,
  "Alt+F4": "is the system's",
  "Alt+Tab": "is the system's",
  "Alt+Shift+Tab": "is the system's",
};

function physicalTabControl(press: KeyPress, onMac: boolean): boolean {
  return onMac && press.ctrlKey && !press.metaKey && ["Tab", "PageUp", "PageDown"].includes(press.code);
}

function keyAt(code: string): string | null {
  const place = /^(?:Key([A-Z])|Digit(\d)|(F\d{1,2}))$/.exec(code);
  if (place) return place[1] ?? place[2] ?? place[3] ?? null;
  return PLACES[code] ?? null;
}

/** The accelerator a press means by where its key sits, whatever the layout types there:
    Ctrl+K with a Russian layout is still `CmdOrCtrl+K`. `null` for a key without a place. */
export function pressedAccelerator(press: KeyPress, onMac: boolean): string | null {
  const key = keyAt(press.code);
  if (key === null || (!onMac && press.getModifierState?.("AltGraph"))) return null;
  const parts: string[] = [];
  if (foreign(press, onMac)) {
    if (!physicalTabControl(press, onMac)) return null;
    parts.push("Ctrl");
  }
  if (primary(press, onMac)) parts.push("CmdOrCtrl");
  if (press.altKey) parts.push("Alt");
  if (press.shiftKey) parts.push("Shift");
  return [...parts, key].join("+");
}

/** What the keymap editor records for a press: the accelerator, or why the window would
    not run it (R-516). `null` for a bare modifier, so the editor keeps waiting. */
export function recordKeys(press: KeyPress, onMac: boolean): Recorded | null {
  if (MODIFIER_KEYS.has(press.key)) return null;

  // AltGr arrives as Ctrl and Alt and types a character; the window lets it through.
  const altGr = !onMac && press.getModifierState?.("AltGraph");
  const macControl = physicalTabControl(press, onMac);
  const foreignHeld = foreign(press, onMac) && !macControl;
  const parts: string[] = [];
  if (macControl) parts.push("Ctrl");
  if (primary(press, onMac) && !altGr) parts.push("CmdOrCtrl");
  if (press.altKey && !altGr) parts.push("Alt");
  if (press.shiftKey) parts.push("Shift");

  const placed = keyAt(press.code);
  const named = placed ?? SHOWN[press.key] ?? (press.key.length === 1 ? press.key.toUpperCase() : press.key);
  const keys = [...parts, named].join("+");

  const extra = altGr ? "AltGr+" : foreignHeld ? (onMac ? "⌃" : "Win+") : "";
  const shown = extra + prettyKeys(keys, onMac);
  if (altGr || foreignHeld || placed === null || !claimable(keys)) return { refused: `${shown} cannot be used here` };
  if (onMac && (keys === "CmdOrCtrl+Tab" || keys === "CmdOrCtrl+Shift+Tab")) {
    return { refused: `${shown} is the system's` };
  }
  const reserved = RESERVED[keys] ?? (onMac && nativeMacShortcut(keys) ? "is kept for the macOS menu" : undefined);
  return reserved ? { refused: `${shown} ${reserved}` } : { keys };
}

const MAC_MODIFIERS: Record<string, string> = { ctrl: "⌃", alt: "⌥", shift: "⇧", cmd: "⌘" };

function partsOf(keys: string, onMac: boolean) {
  const parts = keys.split("+").map((part) => part.trim());
  const key = parts.pop() ?? "";
  const modifiers = parts.map((part) => {
    switch (part.toLowerCase()) {
      case "cmdorctrl": case "commandorcontrol": return onMac ? "cmd" : "ctrl";
      case "command": case "super": case "meta": return "cmd";
      case "control": return "ctrl";
      case "option": return "alt";
      default: return part.toLowerCase();
    }
  });
  return { key, modifiers };
}

/** `Shift+CmdOrCtrl+k` and `CmdOrCtrl+Shift+K` are one chord. */
export function canonicalShortcut(keys: string, onMac: boolean): string {
  const { key, modifiers } = partsOf(keys, onMac);
  return [...new Set(modifiers.sort()), key.toLowerCase()].join("+");
}

const MAC_KEYS: Record<string, string> = {
  left: "←", right: "→", up: "↑", down: "↓", enter: "↩", return: "↩", escape: "⎋", tab: "⇥",
};

export function prettyKeys(keys: string, onMac = ON_MAC): string {
  if (keys === "") return "—";
  if (!onMac) return keys.replace(/CmdOrCtrl|CommandOrControl/gi, "Ctrl");
  const { key, modifiers } = partsOf(keys, true);
  const glyphs = Object.entries(MAC_MODIFIERS).filter(([name]) => modifiers.includes(name)).map(([, glyph]) => glyph);
  return glyphs.join("") + (MAC_KEYS[key.toLowerCase()] ?? key);
}

const MAC_NATIVE = new Set([
  "cmd+tab", "cmd+shift+tab", "cmd+,", "cmd+comma", "cmd+q", "cmd+w", "cmd+m", "cmd+h", "alt+cmd+h",
  "cmd+`", "cmd+shift+`", "cmd+backquote", "cmd+shift+backquote", "cmd+grave", "cmd+shift+grave",
  "cmd+a", "cmd+z", "cmd+shift+z", "cmd+x", "cmd+c", "cmd+v",
]);

export function nativeMacShortcut(keys: string): boolean {
  return MAC_NATIVE.has(canonicalShortcut(keys, true));
}

export function fixedMacCommand(id: string): boolean {
  return id === "settings" || id === "exit" || id === "select-all";
}

/** The keys a command runs by, as its menu item shows them: the user's own from
    Preferences ▸ Keyboard over the shipped ones, Cmd or Ctrl by platform; `undefined` for a
    command without keys. The palette and the toolbar's tips read them here, not from
    strings of their own that a new key left behind. */
export function shortcutOf(id: string, keys: Keymap, onMac: boolean): string | undefined {
  const accelerator = keys[id];
  return accelerator ? prettyKeys(accelerator, onMac) : undefined;
}

/** Palette rows with the keys of their commands, by id (`shortcutOf`). */
export function withShortcuts<T extends { id: string; shortcut?: string }>(
  rows: readonly T[],
  keys: Keymap,
  onMac: boolean,
): T[] {
  return rows.map((row) => {
    const shortcuts = [row.id, `${row.id}-alternate`].map((id) => shortcutOf(id, keys, onMac));
    return { ...row, shortcut: shortcuts.filter(Boolean).join(" / ") || undefined };
  });
}

export function mergeKeymap(stored: unknown): Keymap {
  if (typeof stored !== "object" || stored === null) return {};
  const map: Keymap = {};
  for (const [id, keys] of Object.entries(stored as Record<string, unknown>)) {
    if (typeof keys === "string") map[id] = keys;
  }
  return map;
}
