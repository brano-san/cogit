/** How the Branches panel orders a level: names naturally or by code unit, and optionally
    leaves by the date of their tip (#20). */
export type NameOrder = "natural" | "plain";
export type DateOrder = "off" | "newest" | "oldest";

export interface RefSort {
  names: NameOrder;
  dates: DateOrder;
}

export const DEFAULT_REF_SORT: RefSort = { names: "natural", dates: "off" };

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

function byCodeUnit(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

/** `v1.0.2` before `v1.0.10`; names the collator calls equal (`Main`, `main`) still get
    one fixed order, so the tree does not reshuffle between reloads. */
export function compareNames(a: string, b: string, order: NameOrder): number {
  if (order === "natural") {
    const natural = collator.compare(a, b);
    if (natural !== 0) return natural;
  }
  return byCodeUnit(a, b);
}

/** A ref without a known date sorts after every dated one, whichever way the dates run. */
export function compareDated(
  a: { name: string; date: number | undefined },
  b: { name: string; date: number | undefined },
  sort: RefSort,
): number {
  if (sort.dates !== "off" && a.date !== b.date) {
    if (a.date === undefined) return 1;
    if (b.date === undefined) return -1;
    return sort.dates === "newest" ? b.date - a.date : a.date - b.date;
  }
  return compareNames(a.name, b.name, sort.names);
}

/** Off, newest first, oldest first, off again: one button, as the header has little room. */
export function nextDateOrder(order: DateOrder): DateOrder {
  if (order === "off") return "newest";
  return order === "newest" ? "oldest" : "off";
}

export function parseRefSort(value: unknown): RefSort {
  const raw = (typeof value === "object" && value !== null ? value : {}) as Partial<RefSort>;
  return {
    names: raw.names === "plain" ? "plain" : "natural",
    dates: raw.dates === "newest" || raw.dates === "oldest" ? raw.dates : "off",
  };
}
