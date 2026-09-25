import type { KeyBinding } from "$lib/ipc/bindings";

/** Command id to accelerator. An empty string means the user removed the key. */
export type Keymap = Record<string, string>;

export function effective(bindings: readonly KeyBinding[], overrides: Keymap): Keymap {
  const map: Keymap = {};
  for (const binding of bindings) {
    map[binding.id] = overrides[binding.id] ?? binding.defaultAccelerator ?? "";
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

const MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta", "OS"]);

const NAMED: Record<string, string> = {
  Enter: "Enter",
  " ": "Space",
  Escape: "Esc",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Backspace: "Backspace",
  Delete: "Delete",
  Tab: "Tab",
};

/** `null` for a keypress that is not a shortcut, so the editor keeps waiting. */
export function accelerator(event: KeyboardEvent): string | null {
  if (MODIFIER_KEYS.has(event.key)) return null;

  const parts: string[] = [];
  if (event.ctrlKey || event.metaKey) parts.push("CmdOrCtrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");

  parts.push(keyName(event));
  return parts.join("+");
}

/** A letter or a digit by where it sits (`KeyS`, `Digit7`), not by what the layout types
    there: `Ы` or `&` is no accelerator the menu can read. */
function keyName(event: KeyboardEvent): string {
  const place = /^(?:Key([A-Z])|Digit(\d))$/.exec(event.code ?? "");
  if (place) return place[1] ?? place[2] ?? "";
  return NAMED[event.key] ?? (event.key.length === 1 ? event.key.toUpperCase() : event.key);
}

export function prettyKeys(keys: string, onMac: boolean): string {
  if (keys === "") return "—";
  return keys.replace("CmdOrCtrl", onMac ? "Cmd" : "Ctrl");
}

export function mergeKeymap(stored: unknown): Keymap {
  if (typeof stored !== "object" || stored === null) return {};
  const map: Keymap = {};
  for (const [id, keys] of Object.entries(stored as Record<string, unknown>)) {
    if (typeof keys === "string") map[id] = keys;
  }
  return map;
}
