/** Git refuses most of these itself; the dialog only spares the round trip. */
export function branchNameProblem(name: string, taken: readonly string[]): string | null {
  const trimmed = name.trim();
  if (trimmed === "") return "Enter a name.";
  if (taken.includes(trimmed)) return `${trimmed} already exists.`;
  if (/[\s~^:?*\[\\]/.test(trimmed)) return "A branch name cannot contain spaces or ~^:?*[\\.";
  if (trimmed.startsWith("-") || trimmed.endsWith(".lock")) return "Git will refuse that name.";
  return null;
}

/** A name that is only a label — a group, a preset, a stash message: anything but empty. */
export function textProblem(value: string): string | null {
  return value.trim() === "" ? "Enter a value." : null;
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
