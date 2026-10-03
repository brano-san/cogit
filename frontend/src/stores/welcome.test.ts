import { describe, expect, it } from "vitest";
import { filterRows, mruRows } from "$lib/welcome";
import { WelcomeDialog } from "./welcome.svelte";

const later = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

describe("WelcomeDialog", () => {
  it("opens on the first row with no filter, every row 'checking' until its check ends", () => {
    const dialog = new WelcomeDialog(() => new Promise(() => {}), 50);
    dialog.query = "left over";
    dialog.show(["D:\\a", "D:\\b"]);
    expect(dialog.open).toBe(true);
    expect(dialog.selected).toBe("D:\\a");
    expect(dialog.query).toBe("");
    expect([...dialog.availability.values()]).toEqual(["checking", "checking"]);
  });

  it("opens with nothing selected on an empty list", () => {
    const dialog = new WelcomeDialog(() => Promise.resolve("repository"), 50);
    dialog.show([]);
    expect(dialog.selected).toBeNull();
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

  it("a click selects, the arrows walk the rows the filter shows", () => {
    const rows = mruRows(["a1", "b", "a2"]);
    const dialog = new WelcomeDialog(() => Promise.resolve("repository"), 50);
    dialog.show(rows.map((row) => row.path));
    dialog.select("b");
    expect(dialog.selected).toBe("b");
    dialog.filter("a", filterRows(rows, "a"));
    expect(dialog.selected).toBe("a1");
    dialog.move("down", filterRows(rows, dialog.query));
    expect(dialog.selected).toBe("a2");
  });

  it("keeps the selection on a row after the selected one is removed", () => {
    const rows = mruRows(["a", "b", "c"]);
    const dialog = new WelcomeDialog(() => Promise.resolve("repository"), 50);
    dialog.show(rows.map((row) => row.path));
    dialog.select("b");
    dialog.removing(["b"], rows);
    expect(dialog.selected).toBe("c");
    dialog.removing(["a", "c"], mruRows(["a", "c"]));
    expect(dialog.selected).toBeNull();
  });
});
