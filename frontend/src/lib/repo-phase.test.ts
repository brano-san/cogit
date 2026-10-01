import { describe, expect, it } from "vitest";
import { emptyStateVisible, footerRepository, panelView } from "./repo-phase";
import { CogitError } from "$lib/ipc";

const repo = { root: "C:/repos/one" } as never;
const error = new CogitError({ kind: "invalidState", data: "Not a Git repository" });

describe("panelView", () => {
  it("offers the start screen when nothing is open", () => {
    expect(panelView({ kind: "closed" })).toBe("start");
  });

  it("says so while the first repository is being opened", () => {
    expect(panelView({ kind: "opening", root: "C:/repos/one", repo: null })).toBe("opening");
  });

  // Every ref move re-reads the repository. Blanking the panels for that would make the
  // whole window flash on each commit.
  it("keeps showing the repository while it is being re-read", () => {
    expect(panelView({ kind: "opening", root: "C:/repos/one", repo })).toBe("content");
  });

  it("shows the repository once it is open", () => {
    expect(panelView({ kind: "open", repo })).toBe("content");
  });

  // Item 4: the error belongs in the notification, not in the body of four panels.
  it("keeps the repository when a re-read failed, and says nothing about it", () => {
    expect(panelView({ kind: "failed", root: "C:/repos/one", error, repo })).toBe("content");
  });

  it("falls back to the start screen when the first open failed", () => {
    expect(panelView({ kind: "failed", root: "E:/gone", error, repo: null })).toBe("start");
  });

  it("never asks a panel to render an error of its own", () => {
    const every = [
      { kind: "closed" },
      { kind: "opening", root: "a", repo: null },
      { kind: "opening", root: "a", repo },
      { kind: "open", repo },
      { kind: "failed", root: "a", error, repo },
      { kind: "failed", root: "a", error, repo: null },
    ] as const;
    expect(every.map(panelView)).not.toContain("failed");
  });
});

describe("the window's one empty state", () => {
  it("is up when nothing is open, or an open failed with nothing to show", () => {
    expect(emptyStateVisible({ kind: "closed" }, true)).toBe(true);
    expect(emptyStateVisible({ kind: "failed", root: "a", error, repo: null }, true)).toBe(true);
  });

  it("never flashes while a repository opens or before the session is restored", () => {
    expect(emptyStateVisible({ kind: "opening", root: "a", repo: null }, true)).toBe(false);
    expect(emptyStateVisible({ kind: "closed" }, false)).toBe(false);
  });

  it("is down over an open repository", () => {
    expect(emptyStateVisible({ kind: "open", repo }, true)).toBe(false);
    expect(emptyStateVisible({ kind: "failed", root: "a", error, repo }, true)).toBe(false);
  });
});

describe("the status of an open is said once, in the footer (#4)", () => {
  it("names in the footer the folder being opened, not the status", () => {
    expect(footerRepository({ kind: "opening", root: "C:/repos/dtv_device/", repo: null })).toBe(
      "dtv_device",
    );
  });

  it("names the open repository, and says when there is none", () => {
    expect(footerRepository({ kind: "open", repo: { name: "cogit" } as never })).toBe("cogit");
    expect(footerRepository({ kind: "closed" })).toBe("No repository");
  });
});
