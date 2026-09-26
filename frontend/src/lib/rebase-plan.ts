import type { TodoEntry } from "./ipc";

export function moveEntry(plan: readonly TodoEntry[], from: number, to: number): TodoEntry[] {
  const next = [...plan];
  if (from < 0 || from >= next.length || to < 0 || to >= next.length) return next;
  const [moved] = next.splice(from, 1);
  if (moved) next.splice(to, 0, moved);
  return next;
}

/** Why Git would reject the plan, or `null`. Checked here so the rebase never half-runs. */
export function planProblem(plan: readonly TodoEntry[]): string | null {
  if (plan.length === 0) return "There is nothing to rebase.";

  let kept = 0;
  for (const entry of plan) {
    const merges = entry.action === "squash" || entry.action === "fixup";
    if (merges && kept === 0) {
      return `${entry.action} needs a commit before it to fold into; there is nothing before it.`;
    }
    if (entry.action !== "drop" && !merges) kept += 1;
  }
  if (kept === 0) return "Keep at least one commit.";
  return null;
}

export function previewCount(plan: readonly TodoEntry[]): number {
  return plan.filter(
    (entry) =>
      entry.action !== "drop" && entry.action !== "squash" && entry.action !== "fixup",
  ).length;
}

/** The plan was reordered, re-actioned or reworded since it came: work closing loses. */
export function planChanged(before: readonly TodoEntry[], now: readonly TodoEntry[]): boolean {
  if (before.length !== now.length) return true;
  return now.some((entry, at) => {
    const was = before[at];
    return !was || was.oid !== entry.oid || was.action !== entry.action || was.message !== entry.message;
  });
}
