import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Edge, FrameState } from "./titlebar";

/** The window this page lives in, for a titlebar the page draws. Close is not here: it goes
    through `close_this_window`, the path every window closes by (R-86). */
export const windowControl = {
  minimize: () => getCurrentWindow().minimize(),
  toggleMaximize: () => getCurrentWindow().toggleMaximize(),
  title: () => getCurrentWindow().title(),
  startDragging: () => getCurrentWindow().startDragging(),
  startResize: (edge: Edge) => getCurrentWindow().startResizeDragging(edge),

  async state(): Promise<FrameState> {
    const window = getCurrentWindow();
    const [maximized, fullscreen] = await Promise.all([window.isMaximized(), window.isFullscreen()]);
    return { maximized, fullscreen };
  },

  /** Maximize, restore, snap and fullscreen all arrive as a resize. */
  onResized: (handler: () => void) => getCurrentWindow().onResized(handler),
};
