export type FilterCase = "ignore" | "match" | "smart";

export function ignoreFilterCase(query: string, mode: FilterCase): boolean {
  return mode === "ignore" || (mode === "smart" && !/[\p{Lu}\p{Lt}]/u.test(query));
}

export function containsFilter(query: string, mode: FilterCase): (value: string) => boolean {
  const needle = query.trim();
  if (needle === "") return () => true;
  if (!ignoreFilterCase(needle, mode)) return (value) => value.includes(needle);
  const lower = needle.toLowerCase();
  return (value) => value.toLowerCase().includes(lower);
}
