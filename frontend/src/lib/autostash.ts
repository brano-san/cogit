export interface AutostashSteps {
  ask: (question: string) => Promise<boolean>;
  /** Stash, switch, put the changes back: one operation of the lane, so no other stash
      operation lands between the steps (`switch_with_autostash`, R-521). */
  run: () => Promise<unknown>;
  report: (err: unknown, title: string) => void;
}

/** Stash, switch, put the changes back — what `--autostash` does for rebase and pull.
    `blocked` is what git named as in the way. "declined" leaves git's refusal for the
    caller to report; "done" means the working tree may have changed either way: a refused
    switch has put the changes back, a pop that conflicted leaves the state banner. */
export async function switchWithAutostash(
  branch: string,
  blocked: readonly string[],
  steps: AutostashSteps,
): Promise<"declined" | "done"> {
  const what =
    blocked.length === 0
      ? "Local changes are in the way"
      : `${blocked.length} file(s) are in the way: ${blocked.slice(0, 5).join(", ")}`;
  if (!(await steps.ask(`${what}. Stash them, switch to ${branch}, then put them back?`))) {
    return "declined";
  }
  await steps.run().catch((err) => steps.report(err, "Could not switch branches"));
  return "done";
}
