import { describe, expect, it } from "vitest";
import { NOTIFY_AFTER_MS, notificationFor, type Finished } from "./op-notify";

const on = { enabled: true, success: true, failure: true };
const push: Finished = {
  kind: "push",
  label: "Pushing",
  repoName: "cogit",
  outcome: "success",
  ms: NOTIFY_AFTER_MS + 1,
  summary: "Pushed 3 commits to origin/main",
};

describe("notificationFor", () => {
  it("names the result, the repository and what the toast said", () => {
    expect(notificationFor(push, on, false)).toEqual({
      title: "Push succeeded · cogit",
      body: "Pushed 3 commits to origin/main",
      failed: false,
    });
  });

  it("says nothing in front, for a quick one, or when switched off", () => {
    expect(notificationFor(push, on, true)).toBeNull();
    expect(notificationFor({ ...push, ms: NOTIFY_AFTER_MS - 1 }, on, false)).toBeNull();
    expect(notificationFor(push, { ...on, enabled: false }, false)).toBeNull();
    expect(notificationFor(push, { ...on, success: false }, false)).toBeNull();
  });

  it("tells failures and conflicts apart, under the failure switch", () => {
    expect(notificationFor({ ...push, outcome: "failure" }, on, false)?.title).toBe("Push failed · cogit");
    const merge = { ...push, kind: "merge" as const, outcome: "attention" as const };
    expect(notificationFor(merge, on, false)).toMatchObject({ title: "Merge needs attention · cogit", failed: true });
    expect(notificationFor(merge, { ...on, failure: false }, false)).toBeNull();
  });
});
