import type { ContextItem } from "./ipc";
import { SEPARATOR, item, tidy } from "./context-menu";

/** Rust routes a native menu row with an id of this shape into the one window it was
    opened in, as a `cogit-menu` DOM event (`child_window::on_menu`). */
export const MENU_PREFIX = "child:";

export interface SolverMenuState {
  /** A hunk is under the pointer or current: the take actions have something to act on. */
  hunk: boolean;
  changes: boolean;
  conflicts: boolean;
  tool: boolean;
}

export function actionOf(id: string): string {
  return id.slice(id.lastIndexOf(":") + 1);
}

export function solverMenu(label: string, state: SolverMenuState): ContextItem[] {
  const row = (action: string, text: string, enabled: boolean, keys?: string) =>
    item(`${MENU_PREFIX}${label}:${action}`, text, enabled, keys);
  return tidy([
    row("solver-take-ours", "Take Ours", state.hunk, "CmdOrCtrl+1"),
    row("solver-take-theirs", "Take Theirs", state.hunk, "CmdOrCtrl+2"),
    row("solver-take-ours-theirs", "Take Ours + Theirs", state.hunk, "CmdOrCtrl+3"),
    row("solver-take-theirs-ours", "Take Theirs + Ours", state.hunk, "CmdOrCtrl+4"),
    SEPARATOR,
    row("solver-prev-change", "Previous Change", state.changes, "Shift+F6"),
    row("solver-next-change", "Next Change", state.changes, "F6"),
    row("solver-prev-conflict", "Previous Conflict", state.conflicts, "Shift+F7"),
    row("solver-next-conflict", "Next Conflict", state.conflicts, "F7"),
    SEPARATOR,
    row("solver-external-tool", "Open in External Tool", state.tool),
  ]);
}
