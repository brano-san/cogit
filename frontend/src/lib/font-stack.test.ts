import { describe, expect, it } from "vitest";
import { fontStacks } from "./font-stack";

// Inter came first: installed, it beat the desktop's font that SmartGit used.
describe("fontStacks", () => {
  it("puts the desktop font first and names only installed families on Linux", () => {
    const { ui, mono } = fontStacks("linux", { family: "Ubuntu", installed: ["DejaVu Sans", "Ubuntu Mono", "Ubuntu"] });
    expect(ui).toBe("Ubuntu, \"DejaVu Sans\", sans-serif");
    expect(mono).toBe('"Ubuntu Mono", monospace');
  });

  it("keeps the whole list when Linux could not be asked", () => {
    expect(fontStacks("linux", { family: null, installed: null }).ui).toContain("Cantarell");
  });

  it("uses Segoe UI and Consolas on Windows", () => {
    expect(fontStacks("windows", null)).toEqual({ ui: '"Segoe UI", sans-serif', mono: "Consolas, monospace" });
  });
});
