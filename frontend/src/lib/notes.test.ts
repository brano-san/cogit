import { describe, expect, it } from "vitest";
import { noteTip } from "./notes";

const note = (namespace: string, text: string) => ({ namespace, text });

describe("noteTip", () => {
  it("shows the default namespace bare, keeping line breaks", () => {
    expect(noteTip([note("commits", "a\nb")])).toBe("a\nb");
  });

  it("names every other namespace", () => {
    expect(noteTip([note("ci", "ok"), note("commits", "x")])).toBe("ci:\nok\n\ncommits:\nx");
  });

  it("is empty without notes", () => {
    expect(noteTip([])).toBe("");
  });
});
