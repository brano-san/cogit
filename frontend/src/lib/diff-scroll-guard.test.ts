import { describe, expect, it } from "vitest";
import { ScrollGuard } from "./diff-scroll-guard";

describe("ScrollGuard", () => {
  it("does not write a pane that is there already", () => {
    expect(new ScrollGuard().write("right", 100, 100.4)).toBe(false);
  });

  it("recognises the echo of its own write once, within a pixel", () => {
    const g = new ScrollGuard();
    expect(g.write("right", 0, 240)).toBe(true);
    expect(g.echo("right", 239.5)).toBe(true);
    expect(g.echo("right", 239.5)).toBe(false);
  });

  it("treats a scroll to another place as the user's", () => {
    const g = new ScrollGuard();
    g.write("left", 0, 100);
    expect(g.echo("left", 160)).toBe(false);
  });

  it("keeps the sides apart and forgets on reset", () => {
    const g = new ScrollGuard();
    g.write("left", 0, 100);
    expect(g.echo("right", 100)).toBe(false);
    g.reset();
    expect(g.echo("left", 100)).toBe(false);
  });
});
