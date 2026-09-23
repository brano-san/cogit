import { describe, expect, it } from "vitest";
import { tagNameHint, tagRequest } from "./tag-dialog";

describe("tagNameHint", () => {
  it("asks for a name first", () => {
    expect(tagNameHint("  ", [])).toBe("Enter a name.");
  });

  it("says when the name is taken", () => {
    expect(tagNameHint("v1.0 ", ["v1.0"])).toBe("A tag named 'v1.0' already exists.");
  });

  it("leaves everything else to git check-ref-format", () => {
    expect(tagNameHint("v1..0", ["v1.0"])).toBeNull();
  });
});

describe("tagRequest", () => {
  it("makes an annotated tag from a message", () => {
    expect(tagRequest(" v2 ", "Release two\n\nNotes.\n\n", "abc")).toEqual({
      name: "v2",
      target: "abc",
      message: "Release two\n\nNotes.",
      force: false,
    });
  });

  it("makes a lightweight tag when the message is empty or blank", () => {
    expect(tagRequest("v2", "", "abc").message).toBeNull();
    expect(tagRequest("v2", " \n ", "abc").message).toBeNull();
  });
});
