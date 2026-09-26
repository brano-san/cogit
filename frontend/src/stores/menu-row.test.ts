import { describe, expect, it } from "vitest";
import { MenuRow } from "./menu-row.svelte";

function pending() {
  let close!: () => void;
  const shut = new Promise<void>((resolve) => (close = resolve));
  return { shut, close };
}

// A right click on a row of Repositories left the menu hanging with nothing marking the row
// it was for (item 44 of 25.09).
describe("the row a context menu is open on", () => {
  it("is marked while the menu is up and let go once it closes", async () => {
    const row = new MenuRow();
    const menu = pending();

    const held = row.hold("E:/w/app", () => menu.shut);
    expect(row.key).toBe("E:/w/app");

    menu.close();
    await held;
    expect(row.key).toBeNull();
  });

  it("is the newer row when a second menu opens before the first one answered", async () => {
    const row = new MenuRow();
    const first = pending();
    const second = pending();

    const one = row.hold("E:/w/app", () => first.shut);
    const two = row.hold("E:/w/lib", () => second.shut);
    first.close();
    await one;

    expect(row.key).toBe("E:/w/lib");
    second.close();
    await two;
    expect(row.key).toBeNull();
  });

  it("is let go when the menu fails to open", async () => {
    const row = new MenuRow();

    await expect(row.hold("E:/w/app", () => Promise.reject(new Error("no menu")))).rejects.toThrow("no menu");

    expect(row.key).toBeNull();
  });
});
