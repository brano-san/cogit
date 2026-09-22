import { describe, expect, it } from "vitest";
import { truncateMiddle } from "./path-truncate";

const LOG = "C:\\Users\\brano\\AppData\\Local\\dev.branosan.cogit\\logs\\cogit.log";

describe("truncateMiddle", () => {
  it("leaves a path that fits alone", () => {
    expect(truncateMiddle(LOG, LOG.length)).toBe(LOG);
    expect(truncateMiddle("C:\\a.txt", 80)).toBe("C:\\a.txt");
  });

  it("cuts whole folders out of the middle, keeping the start and the file name", () => {
    expect(truncateMiddle(LOG, 31)).toBe("C:\\Users\\brano\\…\\logs\\cogit.log");
  });

  it("takes folders from the file's end and the root's end in turn while they fit", () => {
    expect(truncateMiddle(LOG, 40)).toBe("C:\\…\\dev.branosan.cogit\\logs\\cogit.log");
  });

  it("never exceeds the budget", () => {
    for (let max = 0; max <= LOG.length; max++) {
      expect(truncateMiddle(LOG, max).length).toBeLessThanOrEqual(max);
    }
  });

  it("works on forward slashes, keeping the root", () => {
    const path = "/home/brano/.config/dev.branosan.cogit/settings.json";
    expect(truncateMiddle(path, 30)).toBe("/home/brano/…/settings.json");
  });

  it("keeps the leading slashes of a UNC path together", () => {
    const path = "\\\\server\\share\\projects\\cogit\\logs\\cogit.log";
    expect(truncateMiddle(path, 30)).toBe("\\\\server\\…\\logs\\cogit.log");
  });

  it("gives up the start before the file name", () => {
    expect(truncateMiddle(LOG, 12)).toBe("…\\cogit.log");
  });

  it("cuts inside the name, keeping its extension, when even the name does not fit", () => {
    expect(truncateMiddle(LOG, 8)).toBe("C:\\….log");
  });

  it("has nothing to show when there is no room at all", () => {
    expect(truncateMiddle(LOG, 0)).toBe("");
    expect(truncateMiddle(LOG, 1)).toBe("…");
  });
});
