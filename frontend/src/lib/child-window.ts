import { closeThisWindow } from "$lib/ipc";

/** `Esc` and `Ctrl+W` close a window that holds one file. `Ctrl+W` is the main window's
    "close repository", but a compare or merge window has no repository to close. */
export function closesWindow(event: {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
}): boolean {
  if (event.key === "Escape") return true;
  return (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "w";
}

export function onWindowKey(event: KeyboardEvent): void {
  if (!closesWindow(event)) return;
  event.preventDefault();
  void closeThisWindow();
}
