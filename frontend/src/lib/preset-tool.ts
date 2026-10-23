import type { PresetStatus } from "./ipc";

/** The tooltip of "<tool> not found": how to install it and where Cogit looked. */
export function missingToolNote(entry: Pick<PresetStatus, "installHint" | "searched">): string {
  const searched = `Searched: ${entry.searched.join(", ")}`;
  return entry.installHint ? `${entry.installHint}\n${searched}` : searched;
}

/** What a preset still lacks in this repository, one line each, with its tooltip. */
export function presetProblems(
  entry: Pick<PresetStatus, "tool" | "toolPath" | "installHint" | "searched" | "missingConfig">,
): { text: string; tip: string }[] {
  const problems: { text: string; tip: string }[] = [];
  if (entry.tool && entry.toolPath === null) {
    problems.push({ text: `${entry.tool} not found`, tip: missingToolNote(entry) });
  }
  if (entry.missingConfig.length > 0) {
    problems.push({
      text: `No ${entry.missingConfig.join(" / ")}`,
      tip: "Without it the tool falls back to its own defaults, which are not this project's.",
    });
  }
  return problems;
}
