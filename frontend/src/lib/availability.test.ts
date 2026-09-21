import { describe, expect, it } from "vitest";
import { NOTHING, reasonFor, reasons, type Context } from "./availability";

function open(over: Partial<Context> = {}): Context {
  return { ...NOTHING, repository: true, ...over };
}

describe("reasonFor", () => {
  it("lets a command with no requirements through", () => {
    expect(reasonFor({}, NOTHING)).toBeUndefined();
  });

  it("blocks anything needing a repository when none is open", () => {
    expect(reasonFor({ repository: true }, NOTHING)).toBe("No repository is open");
  });

  it("blames the missing repository before the missing remote", () => {
    expect(reasonFor({ remote: true }, NOTHING)).toBe("No repository is open");
  });

  it("blames the remote once a repository is open", () => {
    expect(reasonFor({ remote: true }, open())).toBe("This repository has no remote");
  });

  it("lets a remote command through when there is one", () => {
    expect(reasonFor({ remote: true }, open({ remote: true }))).toBeUndefined();
  });

  it("does not demand a repository for the things that stand alone", () => {
    expect(reasonFor({ undo: true }, NOTHING)).toBe("Nothing to undo");
    expect(reasonFor({ file: true }, NOTHING)).toBe("No file is open in the Diff panel");
  });

  it("names each unmet condition in its own words", () => {
    expect(reasonFor({ branch: true }, open())).toBe("HEAD is not on a branch");
    expect(reasonFor({ commit: true }, open())).toBe("Select a commit first");
    expect(reasonFor({ selection: true }, open())).toBe("No file is ticked");
    expect(reasonFor({ staged: true }, open())).toBe("Nothing is staged");
    expect(reasonFor({ changes: true }, open())).toBe("The working tree is clean");
  });

  it("reports the most fundamental reason when several are unmet", () => {
    expect(reasonFor({ remote: true, staged: true }, open())).toBe(
      "This repository has no remote",
    );
  });

  it("passes a command whose every condition is met", () => {
    const everything: Context = {
      repository: true,
      remote: true,
      selection: true,
      changes: true,
      staged: true,
      commit: true,
      branch: true,
      undo: true,
      file: true,
    };
    expect(reasonFor({ repository: true, remote: true, staged: true }, everything)).toBeUndefined();
  });
});

describe("reasons", () => {
  it("answers for a whole toolbar in one pass", () => {
    const table = {
      pull: { remote: true },
      stage: { selection: true },
      undo: { undo: true },
      about: {},
    };

    expect(reasons(table, NOTHING)).toEqual({
      pull: "No repository is open",
      stage: "No repository is open",
      undo: "Nothing to undo",
      about: undefined,
    });
  });

  it("opens up as the state fills in", () => {
    const table = { pull: { remote: true }, stage: { selection: true } };
    const ready = open({ remote: true, selection: true });

    expect(reasons(table, ready)).toEqual({ pull: undefined, stage: undefined });
  });
});
