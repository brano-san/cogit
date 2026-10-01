/** A canvas cannot read `var(--x)`: it asks the computed style once per theme change and
    keeps the answer. `applyTheme` empties the cache. */
let cache = new Map<string, string>();

export function resetTokenColors(): void {
  cache = new Map();
}

/** `--graph-line`, `--status-add`…; `currentColor` when the token is not set. */
export function tokenColor(name: string): string {
  const known = cache.get(name);
  if (known) return known;
  const value =
    typeof document === "undefined" ? "" : getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  if (!value) return "currentColor";
  cache.set(name, value);
  return value;
}
