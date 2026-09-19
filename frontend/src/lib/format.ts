/**
 * Pure presentation helpers.
 *
 * Kept out of the stores so they can be tested without a Svelte runtime, which is the
 * split `doc/09-testing.md` asks for: logic and formatting are tested, markup is not.
 */

import type { Branch, Head } from "./ipc";

/** How many hex characters to show when a full OID would not fit. */
const SHORT_OID = 7;

/**
 * One-line description of HEAD for the status bar.
 *
 * All three shapes get their own wording: a detached or unborn HEAD must never read
 * like an ordinary branch, because the available actions differ.
 */
export function headLabel(head: Head | null | undefined): string {
  if (!head) return "—";
  switch (head.kind) {
    case "branch":
      return head.name;
    case "detached":
      return `detached at ${head.oid.slice(0, SHORT_OID)}`;
    case "unborn":
      return `${head.name} (unborn)`;
  }
}

/**
 * Groups branches for the References panel.
 *
 * Order is left exactly as the backend produced it — sorting there and here would be
 * two sources of truth for the same decision.
 */
export function splitBranches(branches: Branch[]): { local: Branch[]; remote: Branch[] } {
  return {
    local: branches.filter((b) => b.kind === "local"),
    remote: branches.filter((b) => b.kind === "remote"),
  };
}
