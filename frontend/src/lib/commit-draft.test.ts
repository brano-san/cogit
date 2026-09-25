import { describe, expect, it } from "vitest";
import { hasOwnText } from "./commit-draft";

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
