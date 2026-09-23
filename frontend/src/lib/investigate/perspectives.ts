import type { OriginReport } from "$lib/ipc/investigate";

export type Perspective = "log" | "diff" | "blame" | "blameOrigins" | "origins";

export interface PerspectiveInfo {
  id: Perspective;
  label: string;
  shortcut: string;
  hint: string;
}

/** DeepGit's five perspectives, in its order. */
export const PERSPECTIVES: readonly PerspectiveInfo[] = [
  { id: "log", label: "Log", shortcut: "Ctrl+1", hint: "The commit and every file it changed" },
  { id: "diff", label: "Diff", shortcut: "Ctrl+2", hint: "What the commit changed in this file" },
  { id: "blame", label: "Blame", shortcut: "Ctrl+3", hint: "The file with the origin of each line" },
  {
    id: "blameOrigins",
    label: "Blame+Origins",
    shortcut: "Ctrl+4",
    hint: "Blame beside the origin candidates of the selected line",
  },
  {
    id: "origins",
    label: "Origins",
    shortcut: "Ctrl+5",
    hint: "The origin candidates and the chosen one compared with the lines",
  },
];

export interface Panels {
  log: boolean;
  diff: boolean;
  blame: boolean;
  candidates: boolean;
  origin: boolean;
}

export function panelsOf(perspective: Perspective): Panels {
  return {
    log: perspective === "log",
    diff: perspective === "diff",
    blame: perspective === "blame" || perspective === "blameOrigins",
    candidates: perspective === "blameOrigins" || perspective === "origins",
    origin: perspective === "origins",
  };
}

/** Whether picking a line should start the origin search. */
export function searchesOrigins(perspective: Perspective): boolean {
  return perspective !== "log" && perspective !== "diff";
}

/** As DeepGit: when the best origin lies elsewhere, plain Blame gives way to
    Blame+Origins so the candidates can be compared instead of jumped to. */
export function perspectiveAfterSearch(
  perspective: Perspective,
  report: OriginReport | null,
): Perspective {
  if (perspective !== "blame" || !report) return perspective;
  const best = report.candidates[report.best];
  return best && (best.kind === "moved" || best.kind === "copied") ? "blameOrigins" : perspective;
}
