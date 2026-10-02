/** What the missing-git dialog says; the rules live here so they can be tested (R-700). */

export const GIT_DOWNLOAD_URL = "https://git-scm.com/downloads";

export type Os = "windows" | "mac" | "linux";

export function osOf(platform: string): Os {
  if (platform.startsWith("Win")) return "windows";
  if (platform.startsWith("Mac")) return "mac";
  return "linux";
}

/** One line to get git on this system without a browser. */
export function installHint(os: Os): string {
  switch (os) {
    case "windows":
      return "winget install --id Git.Git -e";
    case "mac":
      return "xcode-select --install   (or: brew install git)";
    default:
      return "sudo apt install git   (Fedora: sudo dnf install git, Arch: sudo pacman -S git)";
  }
}

/** Why Cogit needs it, in the words the dialog and the failed-command notice share. */
export const WHY_GIT =
  "Cogit reads history without git, but every change goes through the git program: commit, checkout, merge, push, pull and the rest.";

export function oldGitText(version: string, minimum: string): string {
  return `Git ${version} works, but Cogit is tested with git ${minimum} or newer. Some operations may behave differently or fail; nothing is disabled.`;
}

/** A verdict on one path the user picked or clicked. */
export type Picked =
  | { state: "checking"; path: string }
  | { state: "ok"; path: string; version: string }
  | { state: "bad"; path: string; reason: string };

/** Added under git's own words when a command could not start git at all. */
export const GIT_NOT_FOUND_HINT = "Cogit needs git for every change it makes. Use Fix… to point to git or install it.";

/** The notice body without the hint added to it: the dialog shows git's words only. */
export function gitReasonOf(body: string): string {
  return body.replace(`
${GIT_NOT_FOUND_HINT}`, "");
}
