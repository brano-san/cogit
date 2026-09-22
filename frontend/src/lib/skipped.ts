import type { SkippedRef } from "$lib/ipc";

const PREFIXES = ["refs/tags/", "refs/heads/", "refs/remotes/"];

function shortName(name: string): string {
  const prefix = PREFIXES.find((candidate) => name.startsWith(candidate));
  return prefix ? name.slice(prefix.length) : name;
}

export function describeSkipped(skipped: readonly SkippedRef[]): string | null {
  if (skipped.length === 0) return null;
  const parts = skipped.map((ref) => `${shortName(ref.name)} — ${ref.reason}`);
  return `Not in the graph: ${parts.join("; ")}.`;
}
