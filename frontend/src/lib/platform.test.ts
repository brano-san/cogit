import { describe, expect, it } from "vitest";
import { binName, osOf } from "./platform";

describe("platform", () => {
  it("reads the system from navigator.platform", () => {
    expect(osOf("Win32")).toBe("windows");
    expect(osOf("MacIntel")).toBe("mac");
    expect(osOf("Linux x86_64")).toBe("linux");
    expect(osOf("")).toBe("linux");
  });

  it("names the bin the way the system does", () => {
    expect(binName("windows")).toBe("Recycle Bin");
    expect(binName("mac")).toBe("Trash");
    expect(binName("linux")).toBe("Trash");
  });
});
