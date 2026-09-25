export interface AutostashSteps {
  ask: (question: string) => Promise<boolean>;
  stash: () => Promise<unknown>;
  checkout: () => Promise<unknown>;
  /** The stash just made, applied and dropped: `stash@{0}`. */
  pop: () => Promise<unknown>;
  report: (err: unknown, title: string) => void;
}

/** Stash, switch, put the changes back — what `--autostash` does for rebase and pull.
    `blocked` is what git named as in the way. "declined" leaves git's refusal for the
    caller to report; "done" means the working tree may have changed either way. */
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

  try {
    await steps.stash();
  } catch (err) {
    steps.report(err, "Could not stash the changes");
    return "done";
  }
  try {
    await steps.checkout();
  } catch (refused) {
    // The changes are in the stash just made; left there, they would look lost.
    await steps
      .pop()
      .catch((err) => steps.report(err, "Your changes are in stash@{0}: they could not be put back"));
    steps.report(refused, "Could not switch branches");
    return "done";
  }
  // Popping can conflict; the state banner then takes over, which is the honest outcome.
  await steps.pop().catch((err) => steps.report(err, "Could not put the changes back"));
  return "done";
}
