import { describe, expect, it } from "vitest";
import { SEPARATOR, dropStraySeparators, entry, explained, submenu } from "./menu-entries";

describe("explained", () => {
  it("is a plain enabled row when nothing is in the way", () => {
    expect(explained("a", "Pin", null)).toMatchObject({ label: "Pin", enabled: true });
  });

  it("names the reason in the label, since a native menu has no tooltip on a disabled row", () => {
    expect(explained("a", "Pin", "not for a submodule")).toMatchObject({
      label: "Pin (not for a submodule)",
      enabled: false,
    });
  });
});

describe("submenu", () => {
  it("carries its children", () => {
    const menu = submenu("resolve", "Resolve", [entry("t", "Take Theirs"), entry("o", "Take Ours")]);
    expect(menu.children?.map((child) => child.id)).toEqual(["t", "o"]);
    expect(menu.enabled).toBe(true);
  });

  it("is disabled when told so or when it has nothing inside", () => {
    expect(submenu("r", "Resolve", [entry("t", "Take Theirs")], false).enabled).toBe(false);
    expect(submenu("r", "Resolve", []).enabled).toBe(false);
  });
});

describe("dropStraySeparators", () => {
  const shape = (items: ReturnType<typeof dropStraySeparators>): string[] =>
    items.map((item) =>
      item.separator ? "-" : item.children ? `${item.id}[${shape(item.children).join(",")}]` : item.id,
    );

  it("drops leading, trailing and doubled separators at every depth", () => {
    const nested = submenu("m", "Move", [SEPARATOR, entry("x", "X"), SEPARATOR, SEPARATOR, entry("y", "Y"), SEPARATOR]);
    const items = [SEPARATOR, entry("a", "A"), SEPARATOR, SEPARATOR, nested, SEPARATOR];
    expect(shape(dropStraySeparators(items))).toEqual(["a", "-", "m[x,-,y]"]);
  });
});
