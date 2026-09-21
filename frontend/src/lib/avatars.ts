import type { Author, AvatarRow } from "$lib/ipc";

/** The same normalisation Rust hashes with, so both sides agree on who is who. */
export function keyOf(email: string): string {
  return email.trim().toLowerCase();
}

export interface Commitish {
  authorName: string;
  authorEmail: string;
}

/** One entry per author, in the order the rows appear: the top of the window first. */
export function authorsOf(rows: readonly Commitish[]): Author[] {
  const seen = new Set<string>();
  const authors: Author[] = [];
  for (const row of rows) {
    const key = keyOf(row.authorEmail);
    if (key === "" || seen.has(key)) continue;
    seen.add(key);
    authors.push({ name: row.authorName, email: row.authorEmail });
  }
  return authors;
}

/** A row that arrives without a picture never overwrites one that has it: the queue
    answers later than the window does, and the avatar would flicker back to initials. */
export function mergeRows(
  current: ReadonlyMap<string, AvatarRow>,
  incoming: readonly AvatarRow[],
): Map<string, AvatarRow> {
  const next = new Map(current);
  for (const row of incoming) {
    const key = keyOf(row.email);
    const held = next.get(key);
    next.set(key, held?.image && !row.image ? { ...row, image: held.image } : row);
  }
  return next;
}
