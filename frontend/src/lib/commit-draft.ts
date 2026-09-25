/** The lines of `commit.template` that are hints to the writer. `git commit` strips them
    after the editor, and so does Cogit's commit (commit_write.rs); a `#` line of the
    user's own, like `#123 fix`, is a subject and stays. */
function hints(template: string): Set<string> {
  return new Set(
    template
      .split("\n")
      .map((line) => line.trimEnd())
      .filter((line) => line.startsWith("#")),
  );
}

/** The message as it will be committed. */
export function withoutHints(message: string, template: string | null): string {
  if (template === null) return message;
  const hidden = hints(template);
  return message
    .split("\n")
    .filter((line) => !hidden.has(line.trimEnd()))
    .join("\n");
}

/** Something of the user's own to commit: not blank once the hints are gone, and not the
    template left as it was — `git commit` refuses that one as unedited. */
export function hasOwnText(message: string, template: string | null): boolean {
  const own = withoutHints(message, template).trim();
  if (own === "") return false;
  return template === null || own !== withoutHints(template, template).trim();
}
