import { containsFilter, ignoreFilterCase, type FilterCase } from "./filter-case";

/** What the tooltip adds for a row of the one working-tree list (#32). */
export function indexNote(state: "staged" | "partly" | undefined): string {
  if (state === "staged") return " — staged";
  if (state === "partly") return " — partly staged";
  return "";
}

export function fileName(path: string): string {
  const trimmed = path.endsWith("/") ? path.slice(0, -1) : path;
  return trimmed.slice(trimmed.lastIndexOf("/") + 1);
}

export function matchesMask(path: string, mask: string): boolean {
  return compileMask(mask)(path);
}

/** One matcher per filter change, shared by every file in a commit scope. */
export function compileMask(mask: string, mode: FilterCase = "ignore"): (path: string) => boolean {
  const pattern = mask.trim();
  if (!pattern.includes("*") && !pattern.includes("?")) return containsFilter(pattern, mode);
  const folded = ignoreFilterCase(pattern, mode);
  const expression = globToRegExp(folded ? pattern.toLowerCase() : pattern);
  const fullPath = pattern.includes("/");
  return (path) => {
    const subject = fullPath ? path : fileName(path);
    return expression.test(folded ? subject.toLowerCase() : subject);
  };
}

export function globToRegExp(pattern: string): RegExp {
  const escaped = pattern.replace(/[.+^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`^${escaped.replace(/\*/g, ".*").replace(/\?/g, ".")}$`);
}

export const SUBMODULE_TOOLTIP =
  "Submodule — its checked-out commit differs from the one recorded. Staging records the submodule's current commit; changes inside the submodule are not included.";
