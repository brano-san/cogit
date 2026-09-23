import { desktopInfo, type DesktopInfo } from "$lib/ipc/file-menus";

/** Until the backend has answered: a menu shown this early names Explorer and offers no
    Windows shells rather than waiting. */
const GUESS: DesktopInfo = { fileManager: "Explorer", windowsShells: false, gitShell: null, separator: "/" };

/** What the desktop can do here (#36): asked once, since Git Bash does not move. */
class DesktopStore {
  info = $state.raw<DesktopInfo>(GUESS);
  #asked: Promise<DesktopInfo> | null = null;

  load(): Promise<DesktopInfo> {
    this.#asked ??= desktopInfo()
      .then((info) => (this.info = info))
      .catch(() => {
        this.#asked = null;
        return this.info;
      });
    return this.#asked;
  }
}

export const desktop = new DesktopStore();
