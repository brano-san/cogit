import type { StashEntry } from "$lib/ipc";

export type StashTarget = { index: number; oid: string; message: string };

/**
 * The stash a menu or a double click meant. The graph node's oid is the stash that was
 * clicked; the list's number at that moment may already name another one, so the number
 * only finds the entry when the click carried no oid.
 */
export function stashTarget(
  entries: readonly StashEntry[],
  index: number,
  oid: string | null | undefined,
): StashTarget | null {
  const entry = oid ? entries.find((e) => e.oid === oid) : entries.find((e) => e.index === index);
  const wanted = oid || entry?.oid;
  if (!wanted) return null;
  return { index: entry?.index ?? index, oid: wanted, message: entry?.message ?? "" };
}
