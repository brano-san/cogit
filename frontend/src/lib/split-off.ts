function plural(count: number): string {
  return count === 1 ? "1 file" : `${count} files`;
}

/** Why the split cannot run, or `null` when it can. The backend checks the same rules. */
export function splitProblem(
  changed: readonly string[],
  chosen: readonly string[],
  message: string,
): string | null {
  if (chosen.length === 0) return "Choose the files to split off.";
  if (chosen.length >= changed.length) {
    return "Leave at least one file in the original commit.";
  }
  if (message.trim() === "") return "The new commit needs a message.";
  return null;
}

export function splitSummary(
  changed: readonly string[],
  chosen: readonly string[],
  splitFirst: boolean,
): string {
  const rest = plural(changed.length - chosen.length);
  const taken = plural(chosen.length);
  return splitFirst
    ? `${taken} before, ${rest} in the original commit`
    : `${rest} in the original commit, ${taken} after`;
}
