import type { SubmoduleChange } from "$lib/ipc/bindings";

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

/** `git add` records the submodule's current commit only; edits inside it never go with it. */
export function submoduleTooltip(change: SubmoduleChange): string {
  const inside = [change.modified && "modified files", change.untracked && "untracked files"].filter(Boolean);
  const parts = [
    change.newCommits ? "checked-out commit differs from the one the parent records" : null,
    inside.length ? `${inside.join(" and ")} inside` : null,
  ].filter(Boolean);
  const note = change.newCommits
    ? "Staging records the submodule's current commit; changes inside the submodule are not included."
    : "Nothing to stage: the recorded commit is unchanged, and changes inside the submodule cannot be staged from here. Commit them in the submodule.";
  return `Submodule — ${parts.join("; ")}. ${note}`;
}
