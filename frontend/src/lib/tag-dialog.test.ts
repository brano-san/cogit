import { describe, expect, it } from "vitest";
import { tagNameHint, tagPushRemote, tagRequest } from "./tag-dialog";

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

describe("tagPushRemote", () => {
  it("takes the only remote", () => {
    expect(tagPushRemote(["fork"], null)).toBe("fork");
  });

  it("takes the remote of the current branch's upstream", () => {
    expect(tagPushRemote(["origin", "my/fork"], "my/fork/main")).toBe("my/fork");
  });

  it("falls back to origin", () => {
    expect(tagPushRemote(["origin", "fork"], null)).toBe("origin");
    expect(tagPushRemote(["origin", "fork"], "gone/main")).toBe("origin");
  });

  it("leaves the choice to the user when nothing decides it", () => {
    expect(tagPushRemote(["a", "b"], null)).toBeNull();
    expect(tagPushRemote([], null)).toBeNull();
  });
});
