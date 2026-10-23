import { describe, expect, it } from "vitest";
import { missingToolNote, presetProblems } from "./preset-tool";

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

describe("presetProblems", () => {
  const ready = { tool: "cargo", toolPath: "C:/cargo.exe", installHint: null, searched: ["PATH"], missingConfig: [] };

  it("is empty when the tool and a config file are there", () => {
    expect(presetProblems(ready)).toEqual([]);
  });

  it("gives each missing piece its own line", () => {
    const lines = presetProblems({ ...ready, toolPath: null, missingConfig: ["rustfmt.toml", ".rustfmt.toml"] });
    expect(lines.map((line) => line.text)).toEqual(["cargo not found", "No rustfmt.toml / .rustfmt.toml"]);
  });
});
