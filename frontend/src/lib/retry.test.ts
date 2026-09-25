import { describe, expect, it } from "vitest";
import { retryOf } from "./retry";

const pushed = { operation: "Push", repo: "C:/repos/a", command: "git push --progress origin" };

// Retry ran the operation again in whichever repository was on screen, to its primary
// remote: a push refused in A, retried from the output window after moving to B, pushed B.
describe("what Retry repeats", () => {
  it("repeats a network operation in the repository it ran in, to the same remote", () => {
    expect(retryOf(pushed, "C:/repos/a", "origin")).toBe("push");
    expect(retryOf(pushed, "C:\\repos\\a", "origin")).toBe("push");
  });

  it("offers nothing once another repository is on screen", () => {
    expect(retryOf(pushed, "C:/repos/b", "origin")).toBeNull();
    expect(retryOf(pushed, null, "origin")).toBeNull();
  });

  it("offers nothing when the command went to another remote", () => {
    const upstream = { ...pushed, operation: "Fetch", command: "git fetch --progress upstream" };
    expect(retryOf(upstream, "C:/repos/a", "origin")).toBeNull();
  });

  it("offers nothing for what is not a network operation", () => {
    expect(retryOf({ ...pushed, operation: "Commit", command: "git commit -m x" }, "C:/repos/a", "origin")).toBeNull();
  });
});

// operation_label calls every `git push` "Push", and Retry ran a plain push of the current
// branch: a refused Delete Remote Branch, retried, sent unpublished commits instead.
describe("what Retry does not repeat", () => {
  const push = (command: string) => retryOf({ ...pushed, command }, "C:/repos/a", "origin");

  it("repeats a plain push and a plain fetch", () => {
    expect(push("git push --progress origin")).toBe("push");
    expect(
      retryOf({ ...pushed, operation: "Fetch", command: "git fetch --progress --prune origin" }, "C:/repos/a", "origin"),
    ).toBe("fetch");
  });

  it("offers nothing for a push that deletes a branch", () => {
    expect(push("git push --delete origin topic")).toBeNull();
  });

  it("offers nothing for a push of one refspec, a commit or a tag", () => {
    expect(push("git push --progress origin main")).toBeNull();
    expect(push("git push --progress origin 1a2b3c4:refs/heads/topic")).toBeNull();
    expect(push("git push --progress origin refs/tags/v1")).toBeNull();
    expect(push("git push --progress origin --tags")).toBeNull();
  });

  it("offers nothing for a forced push, which a plain one would not repeat", () => {
    expect(push("git push --progress --force-with-lease origin")).toBeNull();
  });
});
