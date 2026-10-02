/** The one place the page tells which system it runs on: everything else asks here. */

export type Os = "windows" | "mac" | "linux";

export function osOf(platform: string): Os {
  if (platform.startsWith("Win")) return "windows";
  if (platform.startsWith("Mac")) return "mac";
  return "linux";
}

export const OS: Os = osOf(typeof navigator === "undefined" ? "" : navigator.platform);

/** `CmdOrCtrl` means ⌘ here. */
export const ON_MAC = OS === "mac";
export const ON_WINDOWS = OS === "windows";

/** Where Delete moves files, in the system's own words. */
export function binName(os: Os): string {
  return os === "windows" ? "Recycle Bin" : "Trash";
}
