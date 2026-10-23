import { describe, expect, it } from "vitest";
import { tagNameHint, tagPushRemote, tagRequest } from "./tag-dialog";

describe("tagNameHint", () => {
  it("asks for a name first", () => {
    expect(tagNameHint("  ", [])).toBe("Enter a name.");
  });

  it("says when the name is taken", () => {
    expect(tagNameHint("v1.0 ", ["v1.0"])).toBe("A tag named 'v1.0' already exists.");
  });

  it("refuses what git check-ref-format refuses for refs/tags/<name>", () => {
    const refused = [
      "release/", "release.", "v1.lock", "a/b.lock/c", "-v1", "/v1", "v1..0", "a//b", "v@{1}", "@",
      ".hidden", "a/.b", "v 1", "v\t1", "v\x7f", "v~1", "v^1", "v:1", "v?", "v*", "v[1", "v\\1",
    ];
    for (const name of refused) expect(tagNameHint(name, []), name).not.toBeNull();
  });

  it("accepts ordinary tag names", () => {
    for (const name of ["v1.0", "release/2026-10", "v1.0-rc.1", "a@b", "feature/x.y"]) {
      expect(tagNameHint(name, []), name).toBeNull();
    }
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
