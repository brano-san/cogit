import type { Perspective } from "./perspectives";

/** The actions of the window's native menu, `MENU` in `src-tauri/src/commands/investigate.rs`.
    Close is answered in Rust and never reaches the page. */
export const ACTIONS = [
  "copy-line",
  "copy-commit-id",
  "copy-path",
  "follow-renames",
  "ignore-whitespace",
  "refresh",
  "back",
  "forward",
  "go-deeper",
  "close-card",
  "previous-change",
  "next-change",
  "newer-version",
  "older-version",
  "perspective-log",
  "perspective-diff",
  "perspective-blame",
  "perspective-blame-origins",
  "perspective-origins",
  "help",
] as const;

export type InvestigateCommand = (typeof ACTIONS)[number];

export function commandOf(action: string): InvestigateCommand | null {
  return (ACTIONS as readonly string[]).includes(action) ? (action as InvestigateCommand) : null;
}

const PERSPECTIVE_OF: Partial<Record<InvestigateCommand, Perspective>> = {
  "perspective-log": "log",
  "perspective-diff": "diff",
  "perspective-blame": "blame",
  "perspective-blame-origins": "blameOrigins",
  "perspective-origins": "origins",
};

export function perspectiveOf(command: InvestigateCommand): Perspective | null {
  return PERSPECTIVE_OF[command] ?? null;
}
