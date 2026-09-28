/** Mouse "back" and "forward" (buttons 3 and 4) navigate the webview's history. WebView2
    has no setting for them, and a Git client has no page to go back to. Second line only:
    keys, zoom, swipe, drops and the context menu are switched off in the host
    (`webview2::harden`). */
export function isHistoryButton(button: number): boolean {
  return button === 3 || button === 4;
}

export function suppressBrowserNavigation(target: Window): () => void {
  const onbutton = (event: MouseEvent) => {
    if (isHistoryButton(event.button)) event.preventDefault();
  };
  const types = ["mousedown", "mouseup", "pointerup", "auxclick"] as const;
  for (const type of types) target.addEventListener(type, onbutton);
  return () => {
    for (const type of types) target.removeEventListener(type, onbutton);
  };
}
