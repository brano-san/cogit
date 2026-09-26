/** Filter texts kept for later, newest first (F-563): SmartGit's Remember Pattern. */
export const MAX_PATTERNS = 30;

/** Text only, trimmed, each once, at most `MAX_PATTERNS`: a hand-edited file cannot break
    the menu. */
export function knownPatterns(value: unknown): string[] | undefined {
  if (!Array.isArray(value)) return undefined;
  const kept: string[] = [];
  for (const entry of value) {
    if (typeof entry !== "string") continue;
    const pattern = entry.trim();
    if (pattern !== "" && !kept.includes(pattern)) kept.push(pattern);
  }
  return kept.slice(0, MAX_PATTERNS);
}

/** On top; one remembered again moves up instead of showing twice. */
export function remember(patterns: readonly string[], text: string): string[] {
  const pattern = text.trim();
  if (pattern === "") return [...patterns];
  return [pattern, ...patterns.filter((each) => each !== pattern)].slice(0, MAX_PATTERNS);
}

export function forget(patterns: readonly string[], text: string): string[] {
  const pattern = text.trim();
  return patterns.filter((each) => each !== pattern);
}

/** Why `Remember Pattern` is off for the text in the field, or `null` when it is on. */
export function rememberBlocked(patterns: readonly string[], text: string): string | null {
  const pattern = text.trim();
  if (pattern === "") return "Type a filter first";
  if (patterns.includes(pattern)) return "Already remembered";
  return null;
}

/** Why `Forget Pattern` is off, or `null`. */
export function forgetBlocked(patterns: readonly string[], text: string): string | null {
  return patterns.includes(text.trim()) ? null : "The text in the field is not remembered";
}
