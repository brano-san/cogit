import type { Overlap } from "./ipc";

const LABELS: Record<Overlap, string> = {
  none: "—",
  slight: "slight",
  heavy: "heavy",
  same: "same files",
};

export function overlapLabel(overlap: Overlap): string {
  return LABELS[overlap];
}

export function overlapTooltip(shared: readonly string[], total: number): string {
  if (shared.length === 0) return "No files in common with the selected commit.";
  const rest = total - shared.length;
  const more = rest > 0 ? `, and ${rest} more` : "";
  return `Also touched: ${shared.join(", ")}${more}`;
}
