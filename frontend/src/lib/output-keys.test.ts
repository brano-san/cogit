import { describe, expect, it } from "vitest";
import { outputKey, type OutputKeyPress } from "./output-keys";

const press = (over: Partial<OutputKeyPress>): OutputKeyPress => ({
  key: "a",
  ctrl: true,
  inside: true,
  handled: false,
  finding: false,
  allSelected: false,
  ...over,
});

// The window listened to the whole page: Ctrl+A in the commit message selected the output,
// and the next Ctrl+C copied the whole git report instead of the word the user picked.
describe("the keys of the output window", () => {
  it("answers its keys while focus is inside it", () => {
    expect(outputKey(press({ key: "a" }))).toBe("select-all");
    expect(outputKey(press({ key: "f" }))).toBe("find");
    expect(outputKey(press({ key: "=" }))).toBe("bigger");
    expect(outputKey(press({ key: "-" }))).toBe("smaller");
    expect(outputKey(press({ key: "c", allSelected: true }))).toBe("copy");
  });

  it("leaves every key pressed elsewhere to whatever has focus there", () => {
    for (const key of ["a", "c", "f", "=", "-", "Escape"]) {
      expect(outputKey(press({ key, inside: false, allSelected: true }))).toBeNull();
    }
  });

  it("copies the selection itself unless the whole output was selected", () => {
    expect(outputKey(press({ key: "c" }))).toBeNull();
  });

  it("closes its search first, then itself, on Esc", () => {
    expect(outputKey(press({ key: "Escape", ctrl: false, finding: true }))).toBe("end-find");
    expect(outputKey(press({ key: "Escape", ctrl: false }))).toBe("close");
  });

  it("does not close on an Esc something else already answered", () => {
    expect(outputKey(press({ key: "Escape", ctrl: false, handled: true }))).toBeNull();
  });
});
