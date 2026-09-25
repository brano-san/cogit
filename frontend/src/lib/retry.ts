export type RetryKind = "push" | "pull" | "fetch";

interface Ran {
  operation: string;
  /** The repository root the command ran in. */
  repo: string;
  command: string;
}

const KINDS: readonly RetryKind[] = ["push", "pull", "fetch"];

const same = (a: string, b: string) => a.replaceAll("\\", "/").toLowerCase() === b.replaceAll("\\", "/").toLowerCase();

/** The flags a plain run of the toolbar's operation carries itself. */
const PLAIN_FLAGS: ReadonlySet<string> = new Set(["--progress", "--prune", "--ff-only"]);

/** The remote of a plain `git … <kind> [flags] [<remote>]`, `null` when it names none;
    `undefined` when the command did more: a refspec, `--delete`, `--tags` or a force is
    not what a new run would do (R-91). */
function plainRemote(command: string, kind: RetryKind): string | null | undefined {
  const words = command.trim().split(/\s+/);
  const at = words.indexOf(kind);
  if (at < 0) return null;
  const rest = words.slice(at + 1);
  if (rest.some((word) => word.startsWith("-") && !PLAIN_FLAGS.has(word))) return undefined;
  const named = rest.filter((word) => !word.startsWith("-"));
  return named.length > 1 ? undefined : (named[0] ?? null);
}

/** Which network operation Retry may run again for a failed command: only in the
    repository it ran in, and only when it went to the remote a new run would use. */
export function retryOf(ran: Ran, root: string | null, remote: string | null): RetryKind | null {
  const kind = KINDS.find((known) => known === ran.operation.toLowerCase());
  if (!kind || root === null || !same(ran.repo, root)) return null;
  const named = plainRemote(ran.command, kind);
  if (named === undefined || (named !== null && named !== remote)) return null;
  return kind;
}
