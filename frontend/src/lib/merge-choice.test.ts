import { describe, expect, it } from "vitest";
import { chooseMerge, type Answer } from "./merge-choice";

const asking = (answer: Answer) => async () => answer;

describe("chooseMerge", () => {
  it("merges without asking when no fast-forward is possible", async () => {
    let asked = false;
    const result = await chooseMerge("feature", "main", async () => false, async () => {
      asked = true;
      return "confirm";
    });
    expect(result).toEqual({ noFastForward: false });
    expect(asked).toBe(false);
  });
  it("fast-forwards on the confirming button", async () => {
    expect(await chooseMerge("feature", "main", async () => true, asking("confirm"))).toEqual({ noFastForward: false });
  });
  it("creates a merge commit on the alternative button", async () => {
    expect(await chooseMerge("feature", "main", async () => true, asking("alternative"))).toEqual({ noFastForward: true });
  });
  it("does nothing on Cancel", async () => {
    expect(await chooseMerge("feature", "main", async () => true, asking(null))).toBeNull();
  });
  it("merges as before when the check fails", async () => {
    const failing = async () => {
      throw new Error("no");
    };
    expect(await chooseMerge("feature", "main", failing, asking(null))).toEqual({ noFastForward: false });
  });
});
