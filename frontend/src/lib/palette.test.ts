import { describe, expect, it } from "vitest";
import { checkedIds, disabledIds, fuzzyScore, rankCommands, type PaletteCommand } from "./palette";
import type { PanelId } from "./perspectives";

function cmd(id: string, title: string, extra: Partial<PaletteCommand> = {}): PaletteCommand {
  return { id, title, run: () => {}, ...extra };
}

describe("fuzzyScore", () => {
  it("matches an exact prefix best", () => {
    expect(fuzzyScore("Push", "pus")).toBeGreaterThan(fuzzyScore("Repush", "pus"));
  });

  it("matches letters spread through the title", () => {
    expect(fuzzyScore("Create Branch", "cb")).toBeGreaterThan(0);
  });

  it("does not match when a letter is missing", () => {
    expect(fuzzyScore("Push", "pxsh")).toBe(0);
  });

  it("ignores case", () => {
    expect(fuzzyScore("Push", "PUSH")).toBeGreaterThan(0);
  });

  it("gives everything a score when the query is empty", () => {
    expect(fuzzyScore("anything", "")).toBeGreaterThan(0);
  });

  it("prefers a word boundary over a letter inside a word", () => {
    expect(fuzzyScore("Stage File", "sf")).toBeGreaterThan(fuzzyScore("Snapshot", "sf"));
  });
});

describe("rankCommands", () => {
  const commands = [cmd("push", "Push"), cmd("pull", "Pull"), cmd("stage", "Stage File")];

  it("keeps only what matches", () => {
    expect(rankCommands(commands, "pus", []).map((c) => c.id)).toEqual(["push"]);
  });

  it("returns everything for an empty query", () => {
    expect(rankCommands(commands, "", [])).toHaveLength(3);
  });

  it("puts recently used commands first when nothing is typed", () => {
    expect(rankCommands(commands, "", ["stage"])[0]?.id).toBe("stage");
  });

  it("lets the query win over recency", () => {
    expect(rankCommands(commands, "pull", ["stage"])[0]?.id).toBe("pull");
  });

  it("keeps unavailable commands in the list so the user learns why", () => {
    const blocked = [cmd("push", "Push", { unavailable: "No remote" })];

    const ranked = rankCommands(blocked, "push", []);

    expect(ranked).toHaveLength(1);
    expect(ranked[0]?.unavailable).toBe("No remote");
  });

  it("sorts available commands above unavailable ones", () => {
    const mixed = [cmd("a", "Same", { unavailable: "nope" }), cmd("b", "Same")];

    expect(rankCommands(mixed, "same", []).map((c) => c.id)).toEqual(["b", "a"]);
  });

  it("matches a synonym as well as the title", () => {
    const withSynonym = [cmd("push", "Push", { synonyms: ["upload", "send"] })];

    expect(rankCommands(withSynonym, "upload", [])).toHaveLength(1);
  });

  it("handles an empty command list", () => {
    expect(rankCommands([], "anything", [])).toEqual([]);
  });
});

describe("disabledIds", () => {
  const cmd = (id: string, unavailable?: string): PaletteCommand => ({
    id,
    title: id,
    unavailable,
    run: () => {},
  });

  it("names the commands that cannot run", () => {
    expect(disabledIds([cmd("push", "No remote"), cmd("open")])).toEqual(["push"]);
  });

  it("is empty when everything is available", () => {
    expect(disabledIds([cmd("open"), cmd("fetch")])).toEqual([]);
  });

  it("is stable in order so the menu is not rebuilt for a reshuffle", () => {
    const one = disabledIds([cmd("b", "x"), cmd("a", "x")]);
    const two = disabledIds([cmd("a", "x"), cmd("b", "x")]);
    expect(one).toEqual(two);
  });
});

describe("checkedIds", () => {
  const off = {
    panels: [] as PanelId[],
    output: false,
    maximized: false,
    overlap: false,
    perspective: "main" as const,
  };

  it("always ticks the active perspective", () => {
    expect(checkedIds(off)).toEqual(["perspective-main"]);
  });

  it("ticks each visible panel", () => {
    const ids = checkedIds({ ...off, panels: ["graph", "files"] });
    expect(ids).toContain("panel-graph");
    expect(ids).toContain("panel-files");
    expect(ids).not.toContain("panel-diff");
  });

  it("ticks the toggles that are on", () => {
    const ids = checkedIds({ ...off, output: true, maximized: true, overlap: true });
    expect(ids).toEqual(
      expect.arrayContaining(["output", "maximize-panel", "overlap", "perspective-main"]),
    );
  });

  it("follows the perspective that is active", () => {
    expect(checkedIds({ ...off, perspective: "review" })).toEqual(["perspective-review"]);
  });

  it("is sorted so an unchanged set produces an identical array", () => {
    const ids = checkedIds({ ...off, panels: ["refs", "diff"], output: true });
    expect(ids).toEqual([...ids].sort());
  });
});
