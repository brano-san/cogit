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
    answers later than the window does, and the avatar would flicker back to initials.
    Merged in place: a copy of the whole table per answer made every avatar on screen
    update, where a reactive map updates only the authors that came. */
export function mergeRows(rows: Map<string, AvatarRow>, incoming: readonly AvatarRow[]): void {
  for (const row of incoming) {
    const key = keyOf(row.email);
    const held = rows.get(key);
    rows.set(key, held?.image && !row.image ? { ...row, image: held.image } : row);
  }
}

/** The same letters `avatars::fallback` draws, so a row that has not been answered yet
    looks like the one that has. Colour is not mirrored: it needs an MD5 the frontend has
    no other use for, and a neutral square until the answer lands reads as "loading". */
export function initialsOf(name: string, email: string): string {
  const words = name.split(/\s+/).filter(Boolean);
  const letters =
    words.length === 0
      ? [...email.trim()].slice(0, 1)
      : words.length === 1
        ? [...words[0]!].slice(0, 1)
        : [...[...words[0]!].slice(0, 1), ...[...words[words.length - 1]!].slice(0, 1)];

  return letters.join("").toUpperCase() || "?";
}
