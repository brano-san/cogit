import { describe, expect, it } from "vitest";
import {
  type CommitBoxState,
  canCommit,
  draftToSave,
  hasOwnText,
  initialMessage,
  messageAfterCommit,
} from "./commit-draft";

const template = "\n\n# Explain why, not what\n# Wrap at 72\n";

// F-103: the field is seeded with the whole template, and its hints are not a message.
describe("hasOwnText", () => {
  it("is false for the template as it was seeded", () => {
    expect(hasOwnText(template, template)).toBe(false);
  });

  it("is false for the hints alone once the blank lines are gone", () => {
    expect(hasOwnText("# Explain why, not what\n# Wrap at 72", template)).toBe(false);
  });

  it("is true once a subject is written above the hints", () => {
    expect(hasOwnText(`Fix the parser${template}`, template)).toBe(true);
  });

  it("keeps a hash line that is not one of the hints", () => {
    expect(hasOwnText("#123\n# Wrap at 72\n", template)).toBe(true);
  });

  it("is false for a template with a line to fill, left untouched", () => {
    const form = "Ticket: \n\n# What changed\n";
    expect(hasOwnText(form, form)).toBe(false);
    expect(hasOwnText("Ticket: 42\n\n# What changed\n", form)).toBe(true);
  });

  it("without a template is only about blank text", () => {
    expect(hasOwnText("  \n", null)).toBe(false);
    expect(hasOwnText("# a heading", null)).toBe(true);
  });
});

describe("the draft around a commit", () => {
  it("opens with the saved draft, else the template", () => {
    expect(initialMessage("half a message", template)).toBe("half a message");
    expect(initialMessage(null, template)).toBe(template);
    expect(initialMessage(null, null)).toBe("");
  });

  // The seeding ran only when the repository or the template changed: after a commit the
  // field stayed empty until a restart, although `git commit` opens the template each time.
  it("is the template again once a commit is made", () => {
    expect(messageAfterCommit(template)).toBe(template);
    expect(messageAfterCommit(null)).toBe("");
  });

  it("keeps nothing for an empty field or the untouched template", () => {
    expect(draftToSave("", template)).toBeNull();
    expect(draftToSave(template, template)).toBeNull();
    expect(draftToSave(`Fix${template}`, template)).toBe(`Fix${template}`);
  });
});

describe("canCommit", () => {
  const box: CommitBoxState = {
    message: "Fix the parser",
    template: null,
    stagedCount: 1,
    amend: false,
    busy: false,
    committing: false,
    scopeEmpty: false,
    unborn: false,
  };

  it("lets a written message with something staged go", () => {
    expect(canCommit(box)).toBe(true);
  });

  // Hooks take seconds, and the list is only marked busy once it is read back: a second
  // Ctrl+Enter meanwhile started another commit ("nothing to commit", or a second amend).
  it("holds while the commit it started is running", () => {
    expect(canCommit({ ...box, committing: true })).toBe(false);
  });

  it("needs something staged, unless amending", () => {
    expect(canCommit({ ...box, stagedCount: 0 })).toBe(false);
    expect(canCommit({ ...box, stagedCount: 0, amend: true })).toBe(true);
  });

  it("holds while the list is read back or the filter hides every staged file", () => {
    expect(canCommit({ ...box, busy: true })).toBe(false);
    expect(canCommit({ ...box, scopeEmpty: true })).toBe(false);
  });

  // With Amend ticked and every staged file filtered out, the button asked about publishing
  // and then did nothing: an empty path list would have taken the hidden files too.
  it("holds Amend too while the filter hides every staged file", () => {
    expect(canCommit({ ...box, amend: true, scopeEmpty: true })).toBe(false);
  });

  // In a repository without commits Amend let the button go with nothing staged: a question
  // about force-pushing a published commit, then git's "You have nothing to amend".
  it("does not count Amend before the first commit", () => {
    expect(canCommit({ ...box, stagedCount: 0, amend: true, unborn: true })).toBe(false);
    expect(canCommit({ ...box, amend: true, unborn: true, scopeEmpty: true })).toBe(false);
    expect(canCommit({ ...box, amend: true, unborn: true })).toBe(true);
  });
});
