import type { ContextItem } from "./ipc";
import { SEPARATOR, item, tidy } from "./context-menu";
import { LAYOUTS, type SolverLayout } from "./solver-geometry";

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
    row("solver-take-base", "Take Base", state.hunk, "CmdOrCtrl+5"),
    SEPARATOR,
    row("solver-prev-change", "Previous Change", state.changes, "Shift+F6"),
    row("solver-next-change", "Next Change", state.changes, "F6"),
    row("solver-prev-conflict", "Previous Conflict", state.conflicts, "Shift+F7"),
    row("solver-next-conflict", "Next Conflict", state.conflicts, "F7"),
    SEPARATOR,
    row("solver-external-tool", "Open in External Tool", state.tool),
  ]);
}

export interface SolverBarState extends SolverMenuState {
  /** The Result differs from the file on disk and nothing is busy. */
  save: boolean;
  /** Nothing is busy: the file can be resolved, kept or deleted. */
  ready: boolean;
  /** Modify/delete: which side lost the file. */
  deleted: { ours: boolean; theirs: boolean } | null;
  layout: SolverLayout;
  baseChanges: boolean;
  whole: boolean;
}

export const BAR_MENUS = ["File", "Edit", "View", "Go To", "Window"] as const;
export type BarMenu = (typeof BAR_MENUS)[number];

/** The window's own menu bar, SmartGit's: the toolbar's actions with their keys. */
export function solverBarMenu(label: string, menu: BarMenu, state: SolverBarState): ContextItem[] {
  const row = (action: string, text: string, enabled: boolean, keys?: string) =>
    item(`${MENU_PREFIX}${label}:${action}`, text, enabled, keys);
  const check = (action: string, text: string, checked: boolean, enabled: boolean) => ({
    ...row(action, text, enabled),
    checked,
  });
  switch (menu) {
    case "File":
      return tidy([
        row("solver-save", "Save", state.save, "CmdOrCtrl+S"),
        row("solver-resolve", "Mark Resolved", state.ready),
        SEPARATOR,
        row("solver-keep-ours", "Keep File (Ours)", state.ready && state.deleted !== null && !state.deleted.ours),
        row("solver-keep-theirs", "Keep File (Theirs)", state.ready && state.deleted !== null && !state.deleted.theirs),
        row("solver-delete-file", "Delete File", state.ready && state.deleted !== null),
        SEPARATOR,
        row("solver-external-tool", "Open in External Tool", state.tool),
      ]);
    case "Edit":
      return tidy([
        row("solver-undo", "Undo", state.ready, "CmdOrCtrl+Z"),
        row("solver-redo", "Redo", state.ready, "CmdOrCtrl+Y"),
        SEPARATOR,
        ...solverMenu(label, state).filter((entry) => entry.id.includes("take")),
      ]);
    case "View":
      return tidy([
        check("solver-base-changes", "Base Changes", state.baseChanges, state.ready),
        SEPARATOR,
        ...LAYOUTS.map((entry) => check(`solver-layout-${entry.id}`, entry.label, state.layout === entry.id, state.ready)),
        SEPARATOR,
        check("solver-whole-file", "Whole File", state.whole, true),
      ]);
    case "Go To":
      return solverMenu(label, state).filter((entry) => /change|conflict/.test(entry.id));
    case "Window":
      return [row("solver-close", "Close", true, "CmdOrCtrl+W")];
  }
}
