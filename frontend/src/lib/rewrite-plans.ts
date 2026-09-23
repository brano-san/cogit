import type { TodoEntry } from "./ipc";

export const ROOT_BASE = "--root";

export function baseBefore(oid: string, parents: readonly string[]): string {
  return parents.length === 0 ? ROOT_BASE : `${oid}^`;
}

function retarget(
  plan: readonly TodoEntry[],
  oid: string,
  change: Partial<TodoEntry>,
): TodoEntry[] | null {
  if (!plan.some((entry) => entry.oid === oid)) return null;
  return plan.map((entry) => (entry.oid === oid ? { ...entry, ...change } : entry));
}

/** Modify: the rebase stops at the commit, so it can be amended, then continued. */
export function modifyPlan(plan: readonly TodoEntry[], oid: string): TodoEntry[] | null {
  return retarget(plan, oid, { action: "edit" });
}

export function rewordPlan(
  plan: readonly TodoEntry[],
  oid: string,
  message: string,
): TodoEntry[] | null {
  return retarget(plan, oid, { action: "reword", message });
}

/** Squash into the parent, which has to open the plan. No message: Git joins both. */
export function squashPlan(plan: readonly TodoEntry[], oid: string): TodoEntry[] | null {
  const at = plan.findIndex((entry) => entry.oid === oid);
  if (at < 1) return null;
  return retarget(plan, oid, { action: "squash", message: null });
}

export function authorProblem(name: string, email: string): string | null {
  if (name.trim() === "") return "Enter a name.";
  if (email.trim() === "") return "Enter an email.";
  if (/[<>\r\n]/.test(name) || /[<>\r\n]/.test(email)) return "Angle brackets are not allowed.";
  return null;
}

export function fullMessage(details: { summary: string; body: string }): string {
  return details.body === "" ? details.summary : `${details.summary}\n\n${details.body}`;
}
