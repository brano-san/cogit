/** Git refuses most of these itself; the dialog only spares the round trip. */
export function branchNameProblem(name: string, taken: readonly string[]): string | null {
  const trimmed = name.trim();
  if (trimmed === "") return "Enter a name.";
  if (taken.includes(trimmed)) return `${trimmed} already exists.`;
  return refNameProblem(trimmed);
}

/** `git check-ref-format` for the short name of a branch or tag (`refs/heads/<name>`,
    `refs/tags/<name>`), plus the leading `-` that `git branch` and `git tag` refuse. The
    backend asks git again on submit; this is the same rule shown while typing. */
export function refNameProblem(name: string): string | null {
  if (/[\x00-\x20\x7f~^:?*[\\]/.test(name)) return "A name cannot contain spaces, control characters or ~ ^ : ? * [ \\.";
  if (name === "@") return "The name cannot be @.";
  if (name.includes("@{")) return "A name cannot contain @{.";
  if (name.includes("..")) return "A name cannot contain two dots in a row.";
  if (name.startsWith("-")) return "A name cannot start with -.";
  if (name.startsWith("/") || name.endsWith("/") || name.includes("//"))
    return "A name cannot start or end with /, or have // in it.";
  if (name.endsWith(".")) return "A name cannot end with a dot.";
  for (const part of name.split("/")) {
    if (part.startsWith(".")) return "No part of a name between slashes can start with a dot.";
    if (part.endsWith(".lock")) return "No part of a name between slashes can end with .lock.";
  }
  return null;
}

/** A name that is only a label — a group, a preset, a stash message: anything but empty. */
export function textProblem(value: string): string | null {
  return value.trim() === "" ? "Enter a value." : null;
}

export const PRESET_NAME_PROBLEM = "Use Latin letters or digits in the name: the preset's id is made of them.";

/** The backend takes only ASCII letters, digits, `-` and `_` in an id. */
export function presetId(name: string): string {
  return name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

export function presetNameProblem(value: string): string | null {
  return textProblem(value) ?? (presetId(value) === "" ? PRESET_NAME_PROBLEM : null);
}

/** The validator decides, an empty value included; without one the value only has to be
    there. */
export function promptProblem(
  value: string,
  validate?: (value: string) => string | null,
): string | null {
  return validate ? validate(value) : textProblem(value);
}

/** A field that may be left empty. */
export function optional(): string | null {
  return null;
}

/** A ref name has no spaces: a space typed or pasted into one is an underscore. Same
    length, so the caret stays; null when there is nothing to change. */
export function typedRefName(value: string, start: number, end: number): { value: string; start: number; end: number } | null {
  return value.includes(" ") ? { value: value.replaceAll(" ", "_"), start, end } : null;
}

/** On a branch, tag or remote name field. Capture runs before `bind:value` reads it. */
export function refNameField(field: HTMLInputElement, on = true) {
  const typed = () => {
    if (!on) return;
    const next = typedRefName(field.value, field.selectionStart ?? 0, field.selectionEnd ?? 0);
    if (!next) return;
    field.value = next.value;
    field.setSelectionRange(next.start, next.end);
  };
  field.addEventListener("input", typed, true);
  return {
    update(next = true) {
      on = next;
    },
    destroy: () => field.removeEventListener("input", typed, true),
  };
}
