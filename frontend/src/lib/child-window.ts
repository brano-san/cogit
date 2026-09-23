import { closeThisWindow } from "$lib/ipc";
import { suppressNativeMenu } from "$lib/native-menu";

interface Keyed {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
}

/** `Esc` and `Ctrl+W` close a window that holds one file. `Ctrl+W` is the main window's
    "close repository", but a compare or merge window has no repository to close. */
export function closesWindow(event: Keyed): boolean {
  if (event.key === "Escape") return true;
  return (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "w";
}

/** `Esc` waits for the rest of the page: an open find bar inside the window takes it
    first and marks it handled. `Ctrl+W` means nothing to anything inside. */
export function whenCloses(event: Keyed): "now" | "unless-handled" | null {
  if (!closesWindow(event)) return null;
  return event.key === "Escape" ? "unless-handled" : "now";
}

export function onWindowKey(event: KeyboardEvent): void {
  const when = whenCloses(event);
  if (when === "now") {
    event.preventDefault();
    void closeThisWindow();
  } else if (when === "unless-handled") {
    // Components inside register their listeners after this one; whether one of them
    // handled the key is known only once the dispatch is over.
    setTimeout(() => {
      if (!event.defaultPrevented) void closeThisWindow();
    }, 0);
  }
}

/** Read by the fallback in the window's HTML, which closes on `Esc` only while nothing
    else can. */
interface ChildGlobals {
  __cogitMounted?: boolean;
}

/** What every child window sets up once: no browser menu, the closing keys, and the flag
    that retires the HTML fallback. Returns the undo. */
export function installChildWindow(win: Window): () => void {
  const stopMenu = suppressNativeMenu(win.document);
  win.addEventListener("keydown", onWindowKey);
  (win as Window & ChildGlobals).__cogitMounted = true;
  return () => {
    stopMenu();
    win.removeEventListener("keydown", onWindowKey);
  };
}
