import { describe, expect, it } from "vitest";
import { keyLetter } from "./key-letter";
import { outputKey } from "./output-keys";

// Chords were read by the character the layout types: with a Russian layout Ctrl+F in the
// diff is «а», Ctrl+Shift+D is «в», and the keys of 11 §7 and the output window stayed silent.
describe("keyLetter", () => {
  it("names a letter key by where it sits", () => {
    expect(keyLetter({ key: "а", code: "KeyF" })).toBe("f");
    expect(keyLetter({ key: "В", code: "KeyD" })).toBe("d");
    expect(keyLetter({ key: "F", code: "KeyF" })).toBe("f");
  });

  it("keeps any other key as the layout reports it", () => {
    expect(keyLetter({ key: "=", code: "Equal" })).toBe("=");
    expect(keyLetter({ key: "Escape", code: "Escape" })).toBe("escape");
    expect(keyLetter({ key: "f" })).toBe("f");
  });

  it("lets the output window find with a Russian layout", () => {
    const press = { key: "а", code: "KeyF", ctrl: true, inside: true, handled: false, finding: false, allSelected: false };
    expect(outputKey(press)).toBe("find");
  });
});
