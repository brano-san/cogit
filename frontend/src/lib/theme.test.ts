import { describe, expect, it } from "vitest";
import {
  SCHEMA,
  THEME_FILES,
  applyVariables,
  cssName,
  cssVariables,
  isTheme,
  resolveTheme,
  validValue,
  type ThemeFile,
} from "./theme";
import type { Theme } from "./settings";

const IDS = Object.keys(THEME_FILES) as Theme[];
const names = Object.keys(SCHEMA).sort();

function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((at) => {
    const value = parseInt(hex.slice(at, at + 2), 16) / 255;
    return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  }) as [number, number, number];
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a: string, b: string): number {
  const [light, dark] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (light + 0.05) / (dark + 0.05);
}

describe("theme files", () => {
  it.each(IDS)("%s has exactly the schema's tokens", (id) => {
    expect(Object.keys(THEME_FILES[id].tokens).sort()).toEqual(names);
  });

  it.each(IDS)("%s has a valid value for every token, solid except focus.ring", (id) => {
    const bad = Object.entries(SCHEMA)
      .filter(([name, { type }]) => !validValue(type, THEME_FILES[id].tokens[name]))
      .map(([name]) => name);
    expect(bad).toEqual([]);
    const alpha = Object.entries(THEME_FILES[id].tokens).filter(
      ([name, value]) => SCHEMA[name]!.type !== "string" && value.length === 9,
    );
    expect(alpha.map(([name]) => name)).toEqual(["focus.ring"]);
  });

  it("describes every token", () => {
    expect(Object.values(SCHEMA).filter((entry) => !entry.description)).toEqual([]);
  });

  it("keeps the two light themes in the light family and the two dark ones in the dark", () => {
    expect(IDS.map((id) => THEME_FILES[id].family)).toEqual(["light", "light", "dark", "dark"]);
  });

  // The text of the main surfaces: WCAG AA for body text.
  it.each(IDS)("%s: primary text reads at 4.5:1 on the editor, panel and chrome", (id) => {
    const { tokens } = THEME_FILES[id];
    for (const surface of ["bg.editor", "bg.panel", "bg.app", "bg.selected"]) {
      expect(contrast(tokens["fg.primary"]!, tokens[surface]!), `${id} fg.primary on ${surface}`).toBeGreaterThanOrEqual(4.5);
    }
  });

  it.each(IDS)("%s: muted text 4.5:1 on app, panel and editor; primary and secondary on every surface; line numbers 3:1", (id) => {
    const { tokens } = THEME_FILES[id];
    const all = ["bg.app", "bg.panel", "bg.editor", "bg.elevated", "bg.input", "bg.hover", "bg.selected", "bg.selectedInactive"];
    const bad: string[] = [];
    const check = (fg: string, bg: string, min: number) => {
      const ratio = contrast(tokens[fg]!, tokens[bg]!);
      if (ratio < min) bad.push(`${fg} on ${bg}: ${ratio.toFixed(2)} < ${min}`);
    };
    for (const bg of ["bg.editor", "bg.panel", "bg.app"]) check("fg.muted", bg, 4.5);
    for (const fg of ["fg.primary", "fg.secondary"]) for (const bg of all) check(fg, bg, 4.5);
    check("diff.lineNumber", "bg.editor", 3);
    expect(bad).toEqual([]);
  });

  // Diff text on its highlight backgrounds. Inside a marked word (*.word) the syntax colours are
  // off and the text is fg.primary (DiffView .word); on the line background (*.line) the syntax
  // colours stay: fg.secondary for comments, graph.lane.0…5 for the rest (app.css .tok-*).
  const WORDS = ["diff.changed.word", "diff.add.word", "diff.del.word"];
  // add.line and del.line are not asserted: their syntax pairs fail on light and light gray (R-629).
  const LINES = ["diff.changed.line"];
  const ratios = (id: Theme, fgs: string[], bgs: string[]) => {
    const { tokens } = THEME_FILES[id];
    const bad: string[] = [];
    for (const fg of fgs)
      for (const bg of bgs) {
        const ratio = contrast(tokens[fg]!, tokens[bg]!);
        if (ratio < 4.5) bad.push(`${fg} on ${bg}: ${ratio.toFixed(2)}`);
      }
    return bad;
  };

  it.each(IDS)("%s: primary text reads at 4.5:1 on the changed, added and deleted word backgrounds", (id) => {
    expect(ratios(id, ["fg.primary"], WORDS)).toEqual([]);
  });

  it.each(IDS)("%s: primary, secondary and syntax colors read at 4.5:1 on the diff line backgrounds", (id) => {
    const syntax = [0, 1, 2, 3, 4, 5].map((n) => `graph.lane.${n}`);
    expect(ratios(id, ["fg.primary", "fg.secondary", ...syntax], LINES)).toEqual([]);
  });

  it.each(IDS)("%s: secondary text, search ink and text on accent read at 4.5:1", (id) => {
    const { tokens } = THEME_FILES[id];
    expect(contrast(tokens["fg.secondary"]!, tokens["bg.panel"]!)).toBeGreaterThanOrEqual(4.5);
    expect(contrast(tokens["search.ink"]!, tokens["search.current"]!)).toBeGreaterThanOrEqual(4.5);
    expect(contrast(tokens["fg.onAccent"]!, tokens["accent"]!)).toBeGreaterThanOrEqual(4.5);
    expect(contrast(tokens["badge.head.fg"]!, tokens["badge.head.bg"]!)).toBeGreaterThanOrEqual(4.5);
  });

  it("is the surfaces that differ: app, panel, editor and elevated are four tones", () => {
    for (const id of IDS) {
      const { tokens } = THEME_FILES[id];
      const tones = new Set(["bg.app", "bg.panel", "bg.editor", "bg.elevated"].map((name) => tokens[name]));
      // Light's editor and elevated are both white by spec; the rest differ.
      expect(tones.size, id).toBeGreaterThanOrEqual(3);
    }
  });
});

describe("token names", () => {
  it("turns dots into dashes and kebab-cases camelCase", () => {
    expect(cssName("diff.add.word")).toBe("--diff-add-word");
    expect(cssName("bg.app")).toBe("--bg-app");
    expect(cssName("bg.selectedInactive")).toBe("--bg-selected-inactive");
    expect(cssName("diff.centerGutter.action.hover")).toBe("--diff-center-gutter-action-hover");
    expect(cssName("fg.onAccent")).toBe("--fg-on-accent");
    expect(cssName("graph.lane.7")).toBe("--graph-lane-7");
  });

  it("makes no two tokens one property", () => {
    expect(new Set(names.map(cssName)).size).toBe(names.length);
  });

  it("leaves the reserved syntax theme out of the CSS", () => {
    const vars = cssVariables(THEME_FILES.dark.tokens);
    expect(vars["--syntax-theme"]).toBeUndefined();
    expect(vars["--bg-editor"]).toBe(THEME_FILES.dark.tokens["bg.editor"]);
    expect(Object.keys(vars)).toHaveLength(names.length - 1);
  });
});

function withoutTokens(id: Theme, ...missing: string[]): Record<Theme, ThemeFile> {
  const tokens = { ...THEME_FILES[id].tokens };
  for (const name of missing) delete tokens[name];
  return { ...THEME_FILES, [id]: { ...THEME_FILES[id], tokens } };
}

describe("resolveTheme", () => {
  it("is silent for a complete theme", () => {
    for (const id of IDS) {
      const { tokens, warnings } = resolveTheme(id);
      expect(warnings).toEqual([]);
      expect(tokens).toEqual(THEME_FILES[id].tokens);
    }
  });

  it("takes a missing token of a gray theme from light, with a warning", () => {
    const { tokens, warnings } = resolveTheme("lightGrey", undefined, withoutTokens("lightGrey", "diff.add.word"));
    expect(tokens["diff.add.word"]).toBe(THEME_FILES.light.tokens["diff.add.word"]);
    expect(warnings).toEqual(["theme lightGrey: token diff.add.word is missing; using light"]);
  });

  it("takes a missing token of a dark gray theme from dark", () => {
    const { tokens, warnings } = resolveTheme("darkGrey", undefined, withoutTokens("darkGrey", "bg.app", "accent"));
    expect(tokens["bg.app"]).toBe(THEME_FILES.dark.tokens["bg.app"]);
    expect(tokens["accent"]).toBe(THEME_FILES.dark.tokens["accent"]);
    expect(warnings).toHaveLength(2);
    expect(warnings[0]).toContain("using dark");
  });

  it("treats a malformed value as missing", () => {
    const files = withoutTokens("dark");
    files.dark.tokens["accent"] = "blue";
    files.dark.tokens["bg.app"] = "#12345";
    const { tokens, warnings } = resolveTheme("dark", undefined, files, SCHEMA);
    expect(tokens["accent"]).toBeUndefined();
    expect(tokens["bg.app"]).toBeUndefined();
    expect(warnings).toHaveLength(2);
    expect(warnings[0]).toContain("invalid value");
  });

  it("leaves a token unset, with a warning, when the base lacks it too", () => {
    const files = withoutTokens("dark", "accent");
    const { tokens, warnings } = resolveTheme("dark", undefined, files);
    expect(tokens["accent"]).toBeUndefined();
    expect(warnings[0]).toContain("left unset");
  });

  it("merges a partial user theme over the chosen one", () => {
    const { tokens, warnings } = resolveTheme("light", { tokens: { accent: "#ff0000", "focus.ring": "#ff000066" } });
    expect(tokens["accent"]).toBe("#ff0000");
    expect(tokens["focus.ring"]).toBe("#ff000066");
    expect(tokens["bg.app"]).toBe(THEME_FILES.light.tokens["bg.app"]);
    expect(warnings).toEqual([]);
  });

  it("ignores user keys that are unknown, malformed or translucent where solid is required", () => {
    const user = { tokens: { nope: "#ffffff", "bg.app": "red", "bg.panel": "#ffffff80", accent: 5 } };
    const { tokens, warnings } = resolveTheme("dark", user);
    expect(tokens).toEqual(THEME_FILES.dark.tokens);
    expect(warnings).toHaveLength(4);
  });

  it("ignores a user theme of the wrong shape, and an empty one without a word", () => {
    for (const user of ["text", 3, null, [], { accent: "#ff0000" }]) {
      const { tokens, warnings } = resolveTheme("dark", user);
      expect(tokens).toEqual(THEME_FILES.dark.tokens);
      expect(warnings).toHaveLength(1);
    }
    expect(resolveTheme("dark", {}).warnings).toEqual([]);
    expect(resolveTheme("dark", undefined).warnings).toEqual([]);
  });
});

describe("applyVariables", () => {
  it("sets the theme name and every property, and a second theme replaces them", () => {
    const set: Record<string, string> = {};
    const root = { dataset: {} as Record<string, string | undefined>, style: { setProperty: (k: string, v: string) => (set[k] = v) } };
    applyVariables(root, "dark", cssVariables(THEME_FILES.dark.tokens));
    expect(root.dataset["theme"]).toBe("dark");
    expect(set["--bg-app"]).toBe(THEME_FILES.dark.tokens["bg.app"]);
    applyVariables(root, "light", cssVariables(THEME_FILES.light.tokens));
    expect(root.dataset["theme"]).toBe("light");
    expect(set["--bg-app"]).toBe(THEME_FILES.light.tokens["bg.app"]);
  });

  it("knows its themes", () => {
    expect(IDS.every(isTheme)).toBe(true);
    expect(isTheme("sepia")).toBe(false);
    expect(isTheme(undefined)).toBe(false);
  });
});

describe("file icon tokens", () => {
  const MIRRORS: Record<string, string> = {
    "file.icon.page": "bg.editor",
    "file.icon.stroke": "fg.muted",
    "file.icon.modified.fill": "diff.del.line",
    "file.icon.modified.stroke": "status.danger",
    "file.icon.conflict.fill": "status.danger",
    "file.icon.overlay.added": "status.info",
    "file.icon.overlay.staged": "status.success",
    "file.icon.overlay.removed": "status.danger",
    "file.icon.overlay.renamed": "status.info",
    "file.icon.overlay.glyph": "fg.onAccent",
  };

  // The schema cannot reference a token, so each theme holds a copy that must not drift.
  it.each(IDS)("%s: every file.icon token equals the token it mirrors", (id) => {
    const { tokens } = THEME_FILES[id];
    const drift = Object.entries(MIRRORS).filter(([name, source]) => tokens[name] !== tokens[source]);
    expect(drift).toEqual([]);
  });
});
