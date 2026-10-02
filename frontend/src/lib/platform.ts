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

/** The modifiers of a key press or a click; a `KeyboardEvent` and a `MouseEvent` are one. */
export interface Modifiers {
  ctrlKey: boolean;
  metaKey: boolean;
}

/** The key `CmdOrCtrl` means: ⌘ on a Mac, Ctrl elsewhere. Holding the other one is not it:
    Win+A is not Ctrl+A, and Ctrl+click on a Mac is the system's right click. */
export function primary(event: Modifiers, onMac: boolean): boolean {
  return onMac ? event.metaKey : event.ctrlKey;
}

/** The other one, which no accelerator holds: Ctrl on a Mac, Win or Super elsewhere. */
export function foreign(event: Modifiers, onMac: boolean): boolean {
  return onMac ? event.ctrlKey : event.metaKey;
}
