import { describe, expect, it } from "vitest";
import { REF_LABEL_MAX, refLabelText, truncateMiddle, truncatePath } from "./truncate";

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

  it("cuts graph labels at one shared length", () => {
    const long = "x".repeat(REF_LABEL_MAX + 5);
    expect(refLabelText(long)).toHaveLength(REF_LABEL_MAX);
    expect(refLabelText(long)).toContain("…");
  });
});
