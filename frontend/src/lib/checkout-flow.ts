import { blockedByLocalChanges } from "./checkout-refusal";
import { CogitError, type CheckoutTarget } from "./ipc";
import type { CheckoutRequest } from "./ref-checkout";

export interface CheckoutSteps {
  /** Another worktree has the branch checked out: true when the user was asked about it
      there, and nothing is to be checked out here. */
  elsewhere: (branch: string) => Promise<boolean>;
  checkout: (target: CheckoutTarget) => Promise<unknown>;
  ask: (question: string) => Promise<boolean>;
  /** Stash, check out, put the changes back: one operation of the lane (R-521). */
  autostash: (target: CheckoutTarget, message: string) => Promise<unknown>;
  report: (err: unknown, title: string) => void;
  /** Reads back what the checkout changed. */
  after: () => Promise<void>;
}

export type CheckoutOutcome = "done" | "elsewhere" | "failed" | "declined";

const FAILED = "Could not check out";

/** The files git named as in the way, or null when it refused for another reason. */
function inTheWay(err: unknown): string[] | null {
  if (!(err instanceof CogitError) || err.detail.kind !== "command") return null;
  return blockedByLocalChanges(err.detail.data.stderr);
}

export function autostashQuestion(what: string, blocked: readonly string[]): string {
  const files =
    blocked.length === 0
      ? "Local changes are in the way"
      : `${blocked.length} file(s) are in the way: ${blocked.slice(0, 5).join(", ")}`;
  return `${files}. Stash them, check out ${what}, then put them back?`;
}

/** What the Checkout dialog chose, carried out. Git is asked first: many checkouts with
    local changes go through, so the stash is offered only once git has refused for them
    (R-54). "done" means the working tree may have changed and was read back. */
export async function runCheckout(request: CheckoutRequest, steps: CheckoutSteps): Promise<CheckoutOutcome> {
  if (request.branch !== null && (await steps.elsewhere(request.branch))) return "elsewhere";
  try {
    await steps.checkout(request.target);
  } catch (err) {
    const blocked = inTheWay(err);
    if (blocked === null) {
      steps.report(err, FAILED);
      return "failed";
    }
    if (!(await steps.ask(autostashQuestion(request.what, blocked)))) {
      steps.report(err, FAILED);
      return "declined";
    }
    await steps
      .autostash(request.target, `cogit: autostash before checking out ${request.what}`)
      .catch((failed) => steps.report(failed, FAILED));
  }
  await steps.after();
  return "done";
}
