export type MenuMove = { kind: "close" } | { kind: "focus"; to: number };

/** A key pressed while a toolbar menu is open. `at` is the item with the focus, if any;
    items that are off are stepped over, and the arrows go round the ends. Enter and Space
    are the focused item's own. */
export function menuKey(key: string, at: number | null, enabled: readonly boolean[]): MenuMove | null {
  if (key === "Escape" || key === "Tab") return { kind: "close" };
  const on = enabled.flatMap((ok, index) => (ok ? [index] : []));
  if (on.length === 0) return null;
  const first = on[0]!;
  const last = on[on.length - 1]!;
  switch (key) {
    case "Home":
      return { kind: "focus", to: first };
    case "End":
      return { kind: "focus", to: last };
    case "ArrowDown":
      return { kind: "focus", to: at === null ? first : (on.find((index) => index > at) ?? first) };
    case "ArrowUp":
      return {
        kind: "focus",
        to: at === null ? last : (on.findLast((index) => index < at) ?? last),
      };
    default:
      return null;
  }
}
