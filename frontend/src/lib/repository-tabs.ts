export function reconcileTabs(order: readonly string[], open: readonly string[], dormant: readonly string[]): string[] {
  const available = new Set([...open, ...dormant]);
  return [...new Set([...order.filter((root) => available.has(root)), ...open])];
}

export function moveTab(order: readonly string[], source: string, target: string): string[] {
  const next = [...order];
  const from = next.indexOf(source);
  const to = next.indexOf(target);
  if (from >= 0 && to >= 0 && from !== to) {
    next.splice(from, 1);
    next.splice(to, 0, source);
  }
  return next;
}

export function cycleTab(order: readonly string[], active: string | null, direction: 1 | -1): string | null {
  const at = active === null ? -1 : order.indexOf(active);
  if (at < 0) return order[direction === 1 ? 0 : order.length - 1] ?? null;
  return order[(at + direction + order.length) % order.length] ?? null;
}

export function neighborAfterClose(order: readonly string[], root: string): string | null {
  const at = order.indexOf(root);
  return at < 0 ? null : order[at + 1] ?? order[at - 1] ?? null;
}

export function tabsToClose(order: readonly string[], root: string, scope: "this" | "others" | "right"): string[] {
  const at = order.indexOf(root);
  if (at < 0) return [];
  if (scope === "this") return [root];
  return scope === "right" ? order.slice(at + 1) : order.filter((item) => item !== root);
}
