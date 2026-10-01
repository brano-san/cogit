/** Pure rules of the confirm-dialog template (doc/06-design-system.md, "Шаблон диалога"). */

export interface DialogAction {
  label: string;
  onclick: () => void;
  /** Rightmost, filled; Enter answers with it. At most one per dialog. */
  primary?: boolean;
  /** Tooltip: what the button does beyond its name. */
  tip?: string;
  disabled?: boolean;
}

/** Which footer button the focus starts on. A destructive primary never gets it: Enter must
    not be the way work is lost, so the focus starts on Cancel. */
export function initialFocus(destructive: boolean, actions: readonly DialogAction[]): "cancel" | number {
  if (destructive) return "cancel";
  const primary = actions.findIndex((action) => action.primary);
  return primary >= 0 ? primary : "cancel";
}

/** The text of a clamped block was cut: its content is taller than its box. */
export function isClamped(scrollHeight: number, clientHeight: number): boolean {
  return scrollHeight > clientHeight + 1;
}

/** `git stash` names its entries `WIP on <branch>: <hash> <subject>` or `On <branch>: <text>`. */
export function parseStashMessage(raw: string): { branch: string | null; message: string } {
  const match = /^(?:WIP on|On) ([^:\n]+): ([\s\S]*)$/.exec(raw);
  return match ? { branch: match[1]!, message: match[2]! } : { branch: null, message: raw };
}
