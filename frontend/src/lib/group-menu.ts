import type { ContextItem } from "./ipc";
import { SEPARATOR, item, tidy } from "./context-menu";

/** The label without the reason `offer` appended to it: `Merge (already in HEAD)`. */
export function plainLabel(label: string): string {
  return label.replace(/ \([^()]*\)$/, "");
}

/** The menu of several rows is the menu of one row with most items off: every item stays
    where it is, so the menus agree, and says why it is off. `live` replaces the items the
    group can do, by the id they have in `template`. */
export function groupVariant(
  template: readonly ContextItem[],
  live: ReadonlyMap<string, ContextItem>,
  reason: string,
): ContextItem[] {
  return tidy(
    template.map((entry) => {
      if (entry.separator) return SEPARATOR;
      return live.get(entry.id) ?? item(entry.id, `${plainLabel(entry.label)} (${reason})`, false);
    }),
  );
}
