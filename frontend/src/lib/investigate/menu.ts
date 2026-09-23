import { PERSPECTIVES, type Perspective } from "./perspectives";

export type InvestigateCommand =
  | "close"
  | "copyCommitId"
  | "copyPath"
  | "copyLine"
  | "followRenames"
  | "ignoreWhitespace"
  | "refresh"
  | "back"
  | "forward"
  | "goDeeper"
  | "closeCard"
  | "previousChange"
  | "nextChange"
  | "newerVersion"
  | "olderVersion"
  | `perspective:${Perspective}`
  | "help";

export interface MenuEntry {
  id: InvestigateCommand;
  label: string;
  shortcut?: string;
  /** A toggle draws a check mark, a radio item a dot. */
  mark?: "check" | "radio";
}

export type MenuLine = MenuEntry | "separator";

export interface Menu {
  label: string;
  items: readonly MenuLine[];
}

/** The window's own menu (R-283): it never carries the main window's. */
export const MENUS: readonly Menu[] = [
  { label: "File", items: [{ id: "close", label: "Close Window", shortcut: "Ctrl+W" }] },
  {
    label: "Edit",
    items: [
      { id: "copyLine", label: "Copy Line" },
      { id: "copyCommitId", label: "Copy Commit ID", shortcut: "Ctrl+Shift+C" },
      { id: "copyPath", label: "Copy File Path" },
    ],
  },
  {
    label: "View",
    items: [
      { id: "followRenames", label: "Follow Renames", mark: "check" },
      { id: "ignoreWhitespace", label: "Ignore Whitespace Changes", mark: "check" },
      "separator",
      { id: "refresh", label: "Refresh", shortcut: "F5" },
    ],
  },
  {
    label: "Go To",
    items: [
      { id: "back", label: "Back", shortcut: "Alt+Left" },
      { id: "forward", label: "Forward", shortcut: "Alt+Right" },
      "separator",
      { id: "goDeeper", label: "Go Deeper", shortcut: "Ctrl+D" },
      { id: "closeCard", label: "Hide Origin Card" },
      "separator",
      { id: "previousChange", label: "Previous Change", shortcut: "Shift+F6" },
      { id: "nextChange", label: "Next Change", shortcut: "F6" },
      "separator",
      { id: "newerVersion", label: "Newer Version", shortcut: "Alt+Up" },
      { id: "olderVersion", label: "Older Version", shortcut: "Alt+Down" },
    ],
  },
  {
    label: "Window",
    items: PERSPECTIVES.map((p) => ({
      id: `perspective:${p.id}` as const,
      label: p.label,
      shortcut: p.shortcut,
      mark: "radio" as const,
    })),
  },
  { label: "Help", items: [{ id: "help", label: "How Investigate Works", shortcut: "F1" }] },
];

const KEY_NAMES: Record<string, string> = {
  left: "arrowleft",
  right: "arrowright",
  up: "arrowup",
  down: "arrowdown",
};

interface Keyish {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

export function matchesShortcut(event: Keyish, shortcut: string): boolean {
  const parts = shortcut.toLowerCase().split("+");
  const key = parts.pop() ?? "";
  const wanted = KEY_NAMES[key] ?? key;
  return (
    event.key.toLowerCase() === wanted &&
    (event.ctrlKey || event.metaKey) === parts.includes("ctrl") &&
    event.shiftKey === parts.includes("shift") &&
    event.altKey === parts.includes("alt")
  );
}

export function commandForKey(event: Keyish): InvestigateCommand | null {
  for (const menu of MENUS) {
    for (const line of menu.items) {
      if (line !== "separator" && line.shortcut && matchesShortcut(event, line.shortcut)) {
        return line.id;
      }
    }
  }
  return null;
}

/** Separators only between items: never first, last or doubled. */
export function visibleLines(items: readonly MenuLine[]): MenuLine[] {
  const out: MenuLine[] = [];
  for (const line of items) {
    if (line === "separator" && (out.length === 0 || out[out.length - 1] === "separator")) continue;
    out.push(line);
  }
  if (out[out.length - 1] === "separator") out.pop();
  return out;
}
