import { describe, expect, it } from "vitest";
import { fontStack } from "./font-stack";

// Inter came first: installed, it beat the desktop's font that SmartGit used.
describe("fontStack", () => {
  it("puts the desktop font first and names only installed families on Linux", () => {
    expect(fontStack("linux", { family: "Ubuntu", installed: ["DejaVu Sans", "Ubuntu"] })).toBe(
      'Ubuntu, "DejaVu Sans", sans-serif',
    );
  });

  it("keeps the whole list when Linux could not be asked", () => {
    expect(fontStack("linux", { family: null, installed: null })).toContain("Cantarell");
  });

  it("uses Segoe UI on Windows", () => {
    expect(fontStack("windows", null)).toBe('"Segoe UI", sans-serif');
  });

  it("puts the family chosen in Preferences before the desktop's", () => {
    expect(fontStack("windows", null, " JetBrains Mono ")).toBe('"JetBrains Mono", "Segoe UI", sans-serif');
    expect(fontStack("mac", null, "Menlo")).toBe("Menlo, -apple-system, system-ui, sans-serif");
  });
});
