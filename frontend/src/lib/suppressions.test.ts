import { describe, expect, it } from "vitest";
import {
  CONFIRM_EXIT,
  CONFIRM_LOCAL_CHECKOUT,
  healthChoiceId,
  parseChoice,
  suppressedChoices,
} from "./suppressions";

describe("suppressedChoices", () => {
  it("lists nothing when every dialog still shows", () => {
    expect(suppressedChoices(true, {})).toEqual([]);
  });

  // Requirement 1.5: the checkbox in the dialog and the one in Settings are one value.
  it("lists the exit question once it has been turned off", () => {
    expect(suppressedChoices(false, {})).toEqual([
      { id: CONFIRM_EXIT, label: "Confirm before exiting", scope: null },
    ]);
  });

  // Item 40: the Checkout dialog of a local branch, turned off by its Don't show again.
  it("lists the local branch's Checkout dialog once it has been turned off", () => {
    expect(suppressedChoices(true, {}, false)).toEqual([
      { id: CONFIRM_LOCAL_CHECKOUT, label: "Check Out dialog for a local branch", scope: null },
    ]);
    expect(parseChoice(CONFIRM_LOCAL_CHECKOUT)).toEqual({ kind: "confirmLocalCheckout" });
  });

  it("lists a warning ignored for a repository, naming the repository", () => {
    const choices = suppressedChoices(true, {
      "E:/Work1/dtv_device": { "ignoreCaseMismatch:false": "core.ignoreCase does not match" },
    });
    expect(choices).toEqual([
      {
        id: healthChoiceId("E:/Work1/dtv_device", "ignoreCaseMismatch:false"),
        label: "core.ignoreCase does not match",
        scope: "dtv_device",
      },
    ]);
  });

  it("puts what applies everywhere before what applies to one repository", () => {
    const choices = suppressedChoices(false, { "C:/r": { a: "A warning" } });
    expect(choices.map((choice) => choice.scope)).toEqual([null, "r"]);
  });
});

describe("parseChoice", () => {
  it("reads back the repository and the warning a choice was made for", () => {
    const id = healthChoiceId("E:/Work1/dtv_device", "danglingWorktree:gone");
    expect(parseChoice(id)).toEqual({
      kind: "health",
      root: "E:/Work1/dtv_device",
      warning: "danglingWorktree:gone",
    });
  });

  it("knows the exit question", () => {
    expect(parseChoice(CONFIRM_EXIT)).toEqual({ kind: "confirmExit" });
  });

  it("survives a repository path with the separator in it", () => {
    const id = healthChoiceId("E:/odd|name", "w");
    expect(parseChoice(id)).toEqual({ kind: "health", root: "E:/odd|name", warning: "w" });
  });
});
