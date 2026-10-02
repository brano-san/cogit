import { desktopInfo, type DesktopInfo } from "$lib/ipc/file-menus";
import { OS } from "$lib/platform";

/** Until `desktop_info` answers: the names `Platform::file_manager` gives, the separator by OS. */
const GUESS: DesktopInfo = {
  fileManager: { windows: "Explorer", mac: "Finder", linux: "File Manager" }[OS],
  windowsShells: false,
  gitShell: null,
  separator: OS === "windows" ? "\\" : "/",
};

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
