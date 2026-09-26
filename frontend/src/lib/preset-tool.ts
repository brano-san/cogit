import type { PresetStatus } from "./ipc";

/** The tooltip of "<tool> not found": how to install it and where Cogit looked. */
export function missingToolNote(entry: Pick<PresetStatus, "installHint" | "searched">): string {
  const searched = `Searched: ${entry.searched.join(", ")}`;
  return entry.installHint ? `${entry.installHint}\n${searched}` : searched;
}
