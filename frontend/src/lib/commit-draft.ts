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

/** What the field opens with: the saved draft, else the template. */
export function initialMessage(saved: string | null, template: string | null): string {
  return saved ?? template ?? "";
}

/** What the field holds once a commit is made: the template again, as the next
    `git commit` would open with it. */
export function messageAfterCommit(template: string | null): string {
  return template ?? "";
}

/** What to keep in storage for the next start; `null` keeps nothing. The untouched
    template is not a draft: kept, it would outlive a change to the template itself. */
export function draftToSave(message: string, template: string | null): string | null {
  return message === "" || message === template ? null : message;
}

export interface CommitBoxState {
  message: string;
  template: string | null;
  stagedCount: number;
  amend: boolean;
  /** The file list is being read back. */
  busy: boolean;
  /** A commit this box started has not returned yet. */
  committing: boolean;
  /** Every staged file is hidden by the filter (T6.8). */
  scopeEmpty: boolean;
  /** HEAD has no commit yet, so there is nothing to amend. */
  unborn: boolean;
}

/** Amend as it will be sent: ticked, and with a commit there to amend. */
export function amends(box: Pick<CommitBoxState, "amend" | "unborn">): boolean {
  return box.amend && !box.unborn;
}

/** Whether the Commit button and Ctrl+Enter may start a commit. Amend too needs a staged
    file in view when some are staged: git has no `--amend --only` without paths. */
export function canCommit(box: CommitBoxState): boolean {
  const amend = amends(box);
  return (
    !box.committing &&
    hasOwnText(box.message, box.template) &&
    (box.stagedCount > 0 || amend) &&
    !box.busy &&
    !box.scopeEmpty
  );
}
