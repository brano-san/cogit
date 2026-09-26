import { blockedByLocalChanges } from "./checkout-refusal";
import { CogitError, type AutostashOutcome, type CheckoutTarget } from "./ipc";
import type { CheckoutRequest } from "./ref-checkout";

export interface CheckoutSteps {
  /** Another worktree has the branch checked out: true when the user was asked about it
      there, and nothing is to be checked out here. */
  elsewhere: (branch: string) => Promise<boolean>;
  checkout: (target: CheckoutTarget) => Promise<unknown>;
  /** The offer to carry the changes over; null declines it (item 46). */
  ask: (question: string) => Promise<{ drop: boolean } | null>;
  /** Stash, check out, apply the stash: one operation of the lane (R-521, R-563). */
  autostash: (target: CheckoutTarget, message: string, drop: boolean) => Promise<AutostashOutcome>;
  report: (err: unknown, title: string) => void;
  inform: (title: string, body: string) => void;
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

/** Where to find the stash that stayed. A conflict's own account comes from git, as the
    failed command's notice ahead of this one. */
export function keptNotice(what: string, outcome: AutostashOutcome): { title: string; body: string } | null {
  if (outcome.kind === "restored") return null;
  if (outcome.clean) {
    return {
      title: "Changes carried over",
      body: `Checked out ${what} with your changes. The stash they were carried in stays in the list as stash@{0}, as asked.`,
    };
  }
  return {
    title: "Changes did not apply cleanly",
    body: `Checked out ${what}, but your changes did not apply cleanly: see what git reported. They stay in the list as stash@{0}, so nothing is lost.`,
  };
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
    const answer = await steps.ask(autostashQuestion(request.what, blocked));
    if (answer === null) {
      steps.report(err, FAILED);
      return "declined";
    }
    await steps
      .autostash(request.target, `cogit: autostash before checking out ${request.what}`, answer.drop)
      .then((outcome) => {
        const kept = keptNotice(request.what, outcome);
        if (kept) steps.inform(kept.title, kept.body);
      })
      .catch((failed) => steps.report(failed, FAILED));
  }
  await steps.after();
  return "done";
}
