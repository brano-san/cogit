import { describe, expect, it } from "vitest";
import { WelcomeDialog } from "./welcome.svelte";

const later = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

describe("WelcomeDialog", () => {
  it("opens on option 3 and the first row, every row 'checking' until its check ends", () => {
    const dialog = new WelcomeDialog(() => new Promise(() => {}), 50);
    dialog.show(["D:\\a", "D:\\b"]);
    expect(dialog.open).toBe(true);
    expect(dialog.selection).toEqual({ option: 3, row: 0 });
    expect([...dialog.availability.values()]).toEqual(["checking", "checking"]);
  });

  it("opens on option 1 with an empty list", () => {
    const dialog = new WelcomeDialog(() => Promise.resolve("repository"), 50);
    dialog.show([]);
    expect(dialog.selection).toEqual({ option: 1, row: null });
  });

  it("marks a missing path when its check ends, and a hung one stays usable", async () => {
    const dialog = new WelcomeDialog(
      (path) => (path === "hung" ? new Promise(() => {}) : Promise.resolve(path === "gone" ? "missing" : "repository")),
      20,
    );
    dialog.show(["ok", "gone", "hung"]);
    await later(60);
    expect(Object.fromEntries(dialog.availability)).toEqual({ ok: "available", gone: "missing", hung: "unknown" });
  });

  it("drops answers that arrive after the dialog was closed or shown again", async () => {
    let release: (kind: "missing") => void = () => {};
    const dialog = new WelcomeDialog(() => new Promise((resolve) => (release = resolve)), 500);
    dialog.show(["x"]);
    dialog.close();
    release("missing");
    await later(5);
    expect(dialog.open).toBe(false);
    expect(dialog.availability.get("x")).toBe("checking");
  });

  it("choosing a row selects option 3", () => {
    const dialog = new WelcomeDialog(() => Promise.resolve("repository"), 50);
    dialog.show(["a", "b", "c"]);
    dialog.choose(1, 3);
    dialog.chooseRow(2);
    expect(dialog.selection).toEqual({ option: 3, row: 2 });
  });

  it("keeps the selection on a row after one is removed", () => {
    const dialog = new WelcomeDialog(() => Promise.resolve("repository"), 50);
    dialog.show(["a", "b"]);
    dialog.chooseRow(1);
    dialog.listChanged(1);
    expect(dialog.selection).toEqual({ option: 3, row: 0 });
    dialog.listChanged(0);
    expect(dialog.selection.option).toBe(1);
  });
});
