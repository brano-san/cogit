/** The webview's own context menu — reload, save as, print, inspect — is a browser's
    menu, and Cogit is not a browser. It is suppressed at the document, so a panel that
    has not got round to its own menu shows nothing rather than Edge's (R-127).

    A panel with a menu of its own calls `preventDefault` first; by the time this runs the
    event is already handled, and it is left alone. */
export function suppressNativeMenu(target: Document): () => void {
  const onmenu = (event: MouseEvent) => {
    if (event.defaultPrevented) return;
    // Text the user can select is text they may want to copy; that menu is the platform's
    // and worth keeping. Everything else in a Git client is a control.
    if (selectionInside(event.target)) return;
    event.preventDefault();
  };

  target.addEventListener("contextmenu", onmenu);
  return () => target.removeEventListener("contextmenu", onmenu);
}

interface Targeted {
  closest(selector: string): unknown;
  ownerDocument: { getSelection(): Selection | null };
}

/** Duck-typed on purpose: this is about what the target can do, and the check has to work
    where there is no DOM to be an instance of. */
function selectionInside(target: EventTarget | null): boolean {
  const element = target as Targeted | null;
  if (!element || typeof element.closest !== "function") return false;
  if (element.closest("input, textarea")) return true;
  const chosen = element.ownerDocument?.getSelection();
  return chosen != null && !chosen.isCollapsed && chosen.toString().trim() !== "";
}
