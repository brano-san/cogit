import { asCogitError, commandReport } from "./notices";

/** What the window says when a step fails: git's own record in full for a command (INV-05),
    the message for anything else. */
export function failureText(err: unknown): string {
  const error = asCogitError(err);
  if (!error) return "";
  return error.detail.kind === "command" ? commandReport(error.detail.data) : error.message;
}
