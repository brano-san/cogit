import { describe, expect, it, vi } from "vitest";
import { applyPreferences, type ApplyHost } from "./preferences-apply";
import { DEFAULT_SETTINGS, type Settings } from "./settings";

function host(open: boolean, start: Settings = DEFAULT_SETTINGS) {
  let current = { ...start };
  const diff = {
    whitespace: start.ignoreWhitespace,
    spec: open ? ({ kind: "workTreeVsIndex" } as const) : null,
    path: open ? "a.txt" : null,
    setWhitespace: vi.fn(async (_repo: unknown, mode: Settings["ignoreWhitespace"]) => {
      diff.whitespace = mode;
    }),
    load: vi.fn(async () => {}),
  };
  const fake: ApplyHost = {
    current: () => current,
    apply: vi.fn(async (next: Settings) => {
      current = { ...next };
    }),
    setKeymap: vi.fn(async () => {}),
    rebuiltMenu: vi.fn(),
    repo: () => 1 as never,
    diff,
  };
  return { fake, diff };
}

const all: Settings = { ...DEFAULT_SETTINGS, ignoreWhitespace: "all" };

// Ignore all whitespace with no file in Diff never reached the diff store, and Cancel put
// the setting back but left the diff computed with the draft's options.
describe("applying Preferences", () => {
  it("hands a whitespace choice to the diff even when no file is open", async () => {
    const { fake, diff } = host(false);

    await applyPreferences(all, {}, fake);

    expect(diff.whitespace).toBe("all");
    expect(diff.setWhitespace).not.toHaveBeenCalled();
  });

  it("re-runs the open diff with the whitespace chosen", async () => {
    const { fake, diff } = host(true);

    await applyPreferences(all, {}, fake);

    expect(diff.setWhitespace).toHaveBeenCalledWith(1, "all");
  });

  it("goes back, diff included, when Cancel applies the settings of the opening", async () => {
    const { fake, diff } = host(true, all);
    const atOpen = { ...DEFAULT_SETTINGS };
    await applyPreferences({ ...all, contextLines: 9 }, {}, fake);
    diff.load.mockClear();

    await applyPreferences(atOpen, {}, fake);

    expect(diff.setWhitespace).toHaveBeenLastCalledWith(1, DEFAULT_SETTINGS.ignoreWhitespace);
    expect(fake.current().contextLines).toBe(DEFAULT_SETTINGS.contextLines);
  });

  it("re-runs the diff for a setting it was computed with", async () => {
    const { fake, diff } = host(true);

    await applyPreferences({ ...DEFAULT_SETTINGS, contextLines: 9 }, {}, fake);

    expect(diff.load).toHaveBeenCalledOnce();
    expect(diff.setWhitespace).not.toHaveBeenCalled();
  });

  it("leaves the diff's own whitespace mode alone for any other setting", async () => {
    const { fake, diff } = host(false);
    diff.whitespace = "trailing";

    await applyPreferences({ ...DEFAULT_SETTINGS, theme: "light" }, { fetch: "F9" }, fake);

    expect(diff.whitespace).toBe("trailing");
    expect(fake.setKeymap).toHaveBeenCalledWith({ fetch: "F9" });
    expect(fake.rebuiltMenu).toHaveBeenCalledOnce();
  });
});
