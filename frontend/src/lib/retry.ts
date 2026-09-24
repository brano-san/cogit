export type RetryKind = "push" | "pull" | "fetch";

interface Ran {
  operation: string;
  /** The repository root the command ran in. */
  repo: string;
  command: string;
}

const KINDS: readonly RetryKind[] = ["push", "pull", "fetch"];

const same = (a: string, b: string) => a.replaceAll("\\", "/").toLowerCase() === b.replaceAll("\\", "/").toLowerCase();

/** The remote a `git <kind> …` line names, when it names one: its first word that is not
    an option. */
function remoteNamed(command: string, kind: RetryKind): string | null {
  const words = command.trim().split(/\s+/);
  const at = words.indexOf(kind);
  if (at < 0) return null;
  return words.slice(at + 1).find((word) => !word.startsWith("-")) ?? null;
}

/** Which network operation Retry may run again for a failed command: only in the
    repository it ran in, and only when it went to the remote a new run would use. */
export function retryOf(ran: Ran, root: string | null, remote: string | null): RetryKind | null {
  const kind = KINDS.find((known) => known === ran.operation.toLowerCase());
  if (!kind || root === null || !same(ran.repo, root)) return null;
  const named = remoteNamed(ran.command, kind);
  if (named !== null && named !== remote) return null;
  return kind;
}
