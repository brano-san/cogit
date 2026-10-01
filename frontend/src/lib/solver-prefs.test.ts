import { describe, expect, it } from "vitest";
import { DEFAULT_PREFS, loadPrefs, parsePrefs, savePrefs } from "./solver-prefs";

describe("the remembered view of the Conflict Solver", () => {
  it("starts from three panes with Base Changes off", () => {
    expect(parsePrefs(null)).toEqual({ layout: "all", baseChanges: false });
  });

  it("reads back what was saved", () => {
    const kept: Record<string, string> = {};
    savePrefs({ layout: "resultBelow", baseChanges: true }, { setItem: (k, v) => void (kept[k] = v) });
    expect(loadPrefs({ getItem: (k) => kept[k] ?? null })).toEqual({ layout: "resultBelow", baseChanges: true });
  });

  it("mends a key that is wrong and keeps the one that is right", () => {
    expect(parsePrefs('{"layout":"sideways","baseChanges":true}')).toEqual({ layout: "all", baseChanges: true });
    expect(parsePrefs('{"layout":"oursResult","baseChanges":"yes"}')).toEqual({ layout: "oursResult", baseChanges: false });
    expect(parsePrefs("not json")).toEqual(DEFAULT_PREFS);
    expect(parsePrefs("null")).toEqual(DEFAULT_PREFS);
  });

  it("copes with a store that throws", () => {
    const blocked = {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    };
    expect(loadPrefs(blocked)).toEqual(DEFAULT_PREFS);
    expect(() => savePrefs(DEFAULT_PREFS, blocked)).not.toThrow();
  });
});
