import { keyLetter } from "$lib/key-letter";
import { ON_MAC, primary } from "$lib/platform";

interface Press {
  key: string;
  code?: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

/** `Ctrl+F`, `Ctrl+G`, `Ctrl+Shift+G`, `F3` and `Shift+F3`: WebView2 opens its own find bar
    on these unless the page claims them. */
export function isBrowserFind(press: Press, onMac: boolean): boolean {
  const ctrl = primary(press, onMac);
  if (press.key === "F3") return !ctrl && !press.altKey;
  const letter = keyLetter(press);
  return ctrl && !press.altKey && (letter === "f" || letter === "g");
}

/** Cogit's searches are its own: Diff's Find, the filters. Where none of them answered the
    key — Ctrl+F outside the Diff panel, over a binary file — the browser's find bar would
    search the page's markup (item 6 of 25.09). Listens after the page, so any search that
    took the key has already marked it handled. */
export function suppressBrowserFind(target: Window): () => void {
  const onkey = (event: KeyboardEvent) => {
    if (!event.defaultPrevented && isBrowserFind(event, ON_MAC)) event.preventDefault();
  };
  target.addEventListener("keydown", onkey);
  return () => target.removeEventListener("keydown", onkey);
}
