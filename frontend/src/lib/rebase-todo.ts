import type { ContextItem, RebaseStep } from "$lib/ipc";

/** The steps a running rebase has left: a verb changed or a step moved (F-745). */
const VERBS = [
  ["pick", "Pick"],
  ["edit", "Edit (stop after it)"],
  ["squash", "Squash into the step before"],
  ["fixup", "Fixup into the step before"],
  ["drop", "Drop"],
] as const;

export const TODO_PREFIX = "rebase-todo-";

/** git writes short verbs too (`p`, `s`, `f`); the menu ticks the long one. */
const LONG: Record<string, string> = { p: "pick", e: "edit", s: "squash", f: "fixup", d: "drop", r: "reword" };

function verbOf(step: RebaseStep): string {
  return LONG[step.action] ?? step.action;
}

export function todoMenu(steps: readonly RebaseStep[], at: number): ContextItem[] {
  const step = steps[at];
  if (!step) return [];
  const verb = verbOf(step);
  return [
    ...VERBS.map(([id, label]) => ({ id: `${TODO_PREFIX}${id}`, label, enabled: id !== verb })),
    { id: "", label: "", enabled: false, separator: true },
    { id: `${TODO_PREFIX}up`, label: "Move Up", enabled: at > 0 },
    { id: `${TODO_PREFIX}down`, label: "Move Down", enabled: at < steps.length - 1 },
  ];
}

/** The list after menu item `id` on step `at`; null when the id is not one of these. "Up"
    is towards the steps git replays first, as the list draws them above. */
export function editedSteps(steps: readonly RebaseStep[], at: number, id: string): RebaseStep[] | null {
  if (!id.startsWith(TODO_PREFIX) || !steps[at]) return null;
  const command = id.slice(TODO_PREFIX.length);
  const next = steps.map((step) => ({ ...step, action: verbOf(step) }));
  if (command === "up" || command === "down") {
    const to = command === "up" ? at - 1 : at + 1;
    if (to < 0 || to >= next.length) return null;
    [next[at], next[to]] = [next[to]!, next[at]!];
    return next;
  }
  if (!VERBS.some(([verb]) => verb === command)) return null;
  next[at] = { ...next[at]!, action: command };
  return next;
}

export function movedSteps(steps: readonly RebaseStep[], from: number, to: number): RebaseStep[] | null {
  if (from === to || !steps[from] || !steps[to]) return null;
  const next = steps.map((step) => ({ ...step, action: verbOf(step) }));
  const [moved] = next.splice(from, 1);
  next.splice(to, 0, moved!);
  return next;
}
