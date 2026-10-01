import { commands, type ContextItem, type MenuNode, type WindowChrome } from "$lib/ipc/bindings";
import { ON_MAC } from "$lib/keymap";
import type { MenuRow } from "$lib/menu-nav";
import type { FrameState } from "$lib/titlebar";
import { windowControl } from "$lib/window-control";
import { barRows, contextRows } from "$lib/web-menu";
import { readText } from "@tauri-apps/plugin-clipboard-manager";

/** What an id of the bar means when it is the page that has to answer: the clipboard rows
    belong to the focused field, the window rows to this window. Anything else is a command,
    and goes the way a native menu event goes. */
const EDITING: Record<string, string> = {
  "edit-undo": "undo",
  "edit-redo": "redo",
  "edit-cut": "cut",
  "edit-copy": "copy",
};

export interface OpenContext {
  rows: MenuRow[];
  x: number;
  y: number;
}

class WebMenus {
  /** Who draws this window's chrome; the platform default until the first answer. */
  chrome = $state.raw<WindowChrome>({ webMenus: false, customTitlebar: false });
  model = $state.raw<MenuNode[]>([]);
  /** The application's own state, once it pushed any: it is fresher than the model's. */
  live = $state.raw<{ disabled: Set<string>; checked: Set<string> } | null>(null);
  frame = $state.raw<FrameState>({ maximized: false, fullscreen: false });
  context = $state.raw<OpenContext | null>(null);
  title = $state("");
  #watching = false;

  bar = $derived.by(() => {
    const maximized = this.frame.maximized;
    const state = this.live ?? (maximized ? { disabled: new Set<string>(), checked: new Set<string>() } : null);
    return barRows(this.model, ON_MAC, state && { ...state, maximized });
  });

  async boot(): Promise<void> {
    try {
      this.chrome = await commands.windowChrome();
      if (this.chrome.webMenus) await this.reloadModel();
      if (this.chrome.customTitlebar || this.chrome.webMenus) void this.watchFrame();
    } catch {
      // No host to ask (a plain browser): the native paths stay.
    }
  }

  /** Maximize, restore and fullscreen arrive as a resize; the grips and buttons follow. */
  async watchFrame(): Promise<void> {
    if (this.#watching) return;
    this.#watching = true;
    const refresh = async () => {
      try {
        this.frame = await windowControl.state();
      } catch {
        // Not a window of ours: the frame stays as it was.
      }
    };
    await refresh();
    await this.refreshTitle();
    try {
      await windowControl.onResized(refresh);
    } catch {
      // No resize events: the buttons keep their last state.
    }
  }

  async refreshTitle(): Promise<void> {
    try {
      this.title = await windowControl.title();
    } catch {
      // The titlebar stays as it was.
    }
  }

  /** After the keymap changed: the bar shows the keys of the rebuilt one. */
  async reloadModel(): Promise<void> {
    if (!this.chrome.webMenus) return;
    try {
      this.model = await commands.menuModel();
    } catch {
      // The bar keeps what it had.
    }
  }

  /** The same signals that update the native items. */
  pushState(disabled: readonly string[], checked: readonly string[]): void {
    this.live = { disabled: new Set(disabled), checked: new Set(checked) };
  }

  openContext(items: readonly ContextItem[], x: number, y: number): void {
    const rows = contextRows(items, ON_MAC);
    this.context = rows.length === 0 ? null : { rows, x, y };
  }

  closeContext(): void {
    this.context = null;
  }

  async run(id: string, restore?: HTMLElement | null): Promise<void> {
    restore?.focus?.();
    const editing = EDITING[id];
    if (editing) {
      document.execCommand(editing);
      return;
    }
    switch (id) {
      case "edit-paste": {
        const text = await readText().catch(() => "");
        if (text) document.execCommand("insertText", false, text);
        return;
      }
      case "window-minimize":
        return void (await windowControl.minimize().catch(() => {}));
      case "window-maximize":
        return void (await windowControl.toggleMaximize().catch(() => {}));
      case "window-close":
        return void (await commands.closeThisWindow().catch(() => {}));
      default:
        await commands.menuCommand(id).catch(() => {});
    }
  }
}

export const webMenus = new WebMenus();
