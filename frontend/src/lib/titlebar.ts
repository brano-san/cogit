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

/** How far the pointer travels, button held, before the bar hands it to the window manager.
    A press alone never does: the manager keeps the button release, WebKitGTK never sees it
    and goes on treating the button as held, so hover, press and the next click of the page
    under the bar break, and a double click loses its second click (R-728). */
export const DRAG_SLOP = 4;

export function pastSlop(from: { x: number; y: number }, to: { x: number; y: number }): boolean {
  return Math.abs(to.x - from.x) > DRAG_SLOP || Math.abs(to.y - from.y) > DRAG_SLOP;
}

/** Only the primary button drags, and a click on a control inside the bar is its own. */
export function startsDrag(event: { button: number; target: unknown }, bar: { contains(node: unknown): boolean }): boolean {
  if (event.button !== 0) return false;
  const target = event.target as { closest?: (selector: string) => unknown } | null;
  return !(target?.closest?.("button, [role='menuitem'], [data-no-drag]") && bar.contains(target));
}
