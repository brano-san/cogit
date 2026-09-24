import { describe, expect, it } from "vitest";
import { retryOf } from "./retry";

const pushed = { operation: "Push", repo: "C:/repos/a", command: "git push --progress origin main" };

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
