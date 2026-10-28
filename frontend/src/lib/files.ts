
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
  const pattern = mask.trim().toLowerCase();
  if (pattern === "") return true;

  if (!pattern.includes("*") && !pattern.includes("?")) {
    return path.toLowerCase().includes(pattern);
  }
  const subject = pattern.includes("/") ? path : fileName(path);
  return globToRegExp(pattern).test(subject.toLowerCase());
}

export function globToRegExp(pattern: string): RegExp {
  const escaped = pattern.replace(/[.+^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`^${escaped.replace(/\*/g, ".*").replace(/\?/g, ".")}$`);
}

export const SUBMODULE_TOOLTIP =
  "Submodule — its checked-out commit differs from the one recorded. Staging records the submodule's current commit; changes inside the submodule are not included.";
