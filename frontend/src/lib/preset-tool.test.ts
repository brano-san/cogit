import { describe, expect, it } from "vitest";
import { missingToolNote } from "./preset-tool";

describe("missingToolNote (F-107)", () => {
  it("says how to install the tool and every place Cogit looked for it", () => {
    expect(
      missingToolNote({ installHint: "rustup component add rustfmt", searched: ["C:/Users/me/.cargo/bin", "PATH"] }),
    ).toBe("rustup component add rustfmt\nSearched: C:/Users/me/.cargo/bin, PATH");
  });

  it("still names the places without a hint", () => {
    expect(missingToolNote({ installHint: null, searched: ["PATH"] })).toBe("Searched: PATH");
  });
});
