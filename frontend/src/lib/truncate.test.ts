import { describe, expect, it } from "vitest";
import { middleCut, truncateMiddle, truncatePath } from "./truncate";

const LOG = "C:\\Users\\brano\\AppData\\Local\\dev.branosan.cogit\\logs\\cogit.log";

describe("truncatePath", () => {
  it("leaves a path that fits alone", () => {
    expect(truncatePath(LOG, LOG.length)).toBe(LOG);
    expect(truncatePath("C:\\a.txt", 80)).toBe("C:\\a.txt");
  });

  it("cuts whole folders out of the middle, keeping the start and the file name", () => {
    expect(truncatePath(LOG, 31)).toBe("C:\\Users\\brano\\…\\logs\\cogit.log");
  });

  it("takes folders from the file's end and the root's end in turn while they fit", () => {
    expect(truncatePath(LOG, 40)).toBe("C:\\…\\dev.branosan.cogit\\logs\\cogit.log");
  });

  it("never exceeds the budget", () => {
    for (let max = 0; max <= LOG.length; max++) {
      expect(truncatePath(LOG, max).length).toBeLessThanOrEqual(max);
    }
  });

  it("works on forward slashes, keeping the root", () => {
    const path = "/home/brano/.config/dev.branosan.cogit/settings.json";
    expect(truncatePath(path, 30)).toBe("/home/brano/…/settings.json");
  });

  it("keeps the leading slashes of a UNC path together", () => {
    const path = "\\\\server\\share\\projects\\cogit\\logs\\cogit.log";
    expect(truncatePath(path, 30)).toBe("\\\\server\\…\\logs\\cogit.log");
  });

  it("gives up the start before the file name", () => {
    expect(truncatePath(LOG, 12)).toBe("…\\cogit.log");
  });

  it("cuts inside the name, keeping its extension, when even the name does not fit", () => {
    expect(truncatePath(LOG, 8)).toBe("C:\\….log");
  });

  it("has nothing to show when there is no room at all", () => {
    expect(truncatePath(LOG, 0)).toBe("");
    expect(truncatePath(LOG, 1)).toBe("…");
  });
});

describe("truncateMiddle", () => {
  it("leaves a label that fits alone", () => {
    expect(truncateMiddle("main", 30)).toBe("main");
  });

  it("keeps the start and the end that tells two branches apart", () => {
    expect(truncateMiddle("feature/14340_new_toolchain", 24)).toBe("feature/143…ew_toolchain");
    expect(truncateMiddle("origin/feature/14339_net_control_opt", 24)).toBe(
      "origin/feat…_control_opt",
    );
  });

  it("never exceeds the budget", () => {
    const label = "origin/feature/14339_net_control_opt";
    for (let max = 0; max <= label.length; max++) {
      expect(truncateMiddle(label, max).length).toBeLessThanOrEqual(max);
    }
  });
});

describe("middleCut", () => {
  it("splits a label into a lead that gives way and a tail that stays", () => {
    const { lead, tail } = middleCut("feature/14340_new_toolchain");

    expect(lead + tail).toBe("feature/14340_new_toolchain");
    expect(tail).toBe("_toolchain");
  });

  it("keeps half of a short label as its tail", () => {
    expect(middleCut("main")).toEqual({ lead: "ma", tail: "in" });
    expect(middleCut("")).toEqual({ lead: "", tail: "" });
  });

  // A label cut to 30 characters first, then again by the row, showed two ellipses.
  it("is the only cut: the lead of a long label is whole, for CSS to shorten", () => {
    const long = `feature/${"x".repeat(40)}_toolchain`;
    const { lead, tail } = middleCut(long);

    expect(lead + tail).toBe(long);
    expect(lead).not.toContain("…");
  });
});

// Half an emoji on each side of the cut drew two replacement marks in the capsule.
describe("a character outside the Basic Multilingual Plane", () => {
  const lone = /[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/;

  it("is never split by the middle cut", () => {
    const { lead, tail } = middleCut("a🔥b");
    expect(lead + tail).toBe("a🔥b");
    expect(lead).not.toMatch(lone);
    expect(tail).not.toMatch(lone);
  });

  it("is never split by a cut to a length", () => {
    for (let max = 0; max <= 12; max++) {
      expect(truncateMiddle("🔥".repeat(12), max)).not.toMatch(lone);
      expect(truncatePath("C:\\🔥🔥🔥\\🔥🔥🔥🔥.txt", max)).not.toMatch(lone);
    }
  });

  it("counts as one character against the length", () => {
    expect(truncateMiddle("🔥🔥🔥", 3)).toBe("🔥🔥🔥");
  });
});
