/** What the page knows of its window. */
export interface FrameState {
  maximized: boolean;
  fullscreen: boolean;
}

export interface ChromeFlags {
  customTitlebar: boolean;
  webMenus: boolean;
}

/** The row on top of the page: the titlebar when the window has no decorations of its own,
    the bare menu bar when only the menus are the page's, nothing in fullscreen. */
export type TopRow = "titlebar" | "menubar" | "none";

export function topRow(chrome: ChromeFlags, state: FrameState, hasMenu: boolean): TopRow {
  if (state.fullscreen) return "none";
  if (chrome.customTitlebar) return "titlebar";
  return chrome.webMenus && hasMenu ? "menubar" : "none";
}

export function maximizeButton(state: FrameState): { title: string; glyph: "maximize" | "restore" } {
  return state.maximized ? { title: "Restore", glyph: "restore" } : { title: "Maximize", glyph: "maximize" };
}

export const EDGES = ["North", "South", "East", "West", "NorthEast", "NorthWest", "SouthEast", "SouthWest"] as const;
export type Edge = (typeof EDGES)[number];

/** The grips a window without decorations needs: none while maximized or fullscreen, where
    there is nothing to drag. */
export function edgesFor(chrome: ChromeFlags, state: FrameState): readonly Edge[] {
  return chrome.customTitlebar && !state.maximized && !state.fullscreen ? EDGES : [];
}

export function edgeCursor(edge: Edge): string {
  switch (edge) {
    case "North":
    case "South":
      return "ns-resize";
    case "East":
    case "West":
      return "ew-resize";
    case "NorthEast":
    case "SouthWest":
      return "nesw-resize";
    default:
      return "nwse-resize";
  }
}

/** Only the primary button drags, and a click on a control inside the bar is its own. */
export function startsDrag(event: { button: number; target: unknown }, bar: { contains(node: unknown): boolean }): boolean {
  if (event.button !== 0) return false;
  const target = event.target as { closest?: (selector: string) => unknown } | null;
  return !(target?.closest?.("button, [role='menuitem'], [data-no-drag]") && bar.contains(target));
}
