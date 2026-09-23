import { describe, expect, it } from "vitest";
import {
  followUp,
  lfsMissingDialog,
  lfsPruneDialog,
  lfsTrackDialog,
  lfsTrackRequest,
  pathFromUrl,
  problemOf,
  submoduleAddDialog,
  submoduleDialog,
  subtreeDialog,
  subtreeRequest,
} from "./remote-dialogs";

const CONTEXT = { prefixes: ["lib", "vendor/ui"], remotes: ["origin", "upstream"] };

describe("submodule dialogs", () => {
  it("preselects the submodule the user pointed at", () => {
    expect(submoduleDialog("reset", ["a", "b"], "b").values.path).toBe("b");
    expect(submoduleDialog("reset", ["a", "b"], null).values.path).toBe("a");
  });

  it("marks every change to a checkout as destructive", () => {
    for (const op of ["reset", "deactivate", "deinit", "unregister"] as const) {
      expect(submoduleDialog(op, ["a"], null).destructive, op).toBe(true);
    }
  });

  it("asks for a URL and a path before adding", () => {
    const spec = submoduleAddDialog();
    expect(problemOf(spec, { url: "", path: "x", branch: "" })).toEqual({
      field: "url",
      message: "Enter repository url",
    });
    expect(problemOf(spec, { url: "https://h/x.git", path: " ", branch: "" })?.field).toBe("path");
    expect(problemOf(spec, { url: "https://h/x.git", path: "libs/x", branch: "" })).toBeNull();
  });

  it("suggests a folder named after the repository", () => {
    expect(pathFromUrl("https://github.com/team/library.git")).toBe("library");
    expect(pathFromUrl("git@github.com:team/ui-kit.git")).toBe("ui-kit");
    expect(pathFromUrl("C:\\repos\\shared\\")).toBe("shared");
    expect(pathFromUrl("")).toBe("");
  });
});

describe("subtree dialogs", () => {
  it("offers the prefixes found in the history, except when adding a new one", () => {
    const merge = subtreeDialog("merge", CONTEXT);
    expect(merge.values.prefix).toBe("lib");
    expect(merge.fields[0]).toMatchObject({ name: "prefix", suggestions: ["lib", "vendor/ui"] });
    const add = subtreeDialog("add", CONTEXT);
    expect(add.values.prefix).toBe("");
    expect(add.fields[0]).toMatchObject({ suggestions: [] });
  });

  it("lets merge go without a repository and makes it a local merge", () => {
    const spec = subtreeDialog("merge", CONTEXT);
    const values = { prefix: "lib", repository: " ", reference: "lib/main", squash: true };
    expect(problemOf(spec, values)).toBeNull();
    expect(subtreeRequest("merge", values)).toEqual({
      kind: "merge",
      prefix: "lib",
      repository: null,
      reference: "lib/main",
      squash: true,
    });
  });

  it("needs a repository to add or push", () => {
    for (const action of ["add", "push"] as const) {
      const spec = subtreeDialog(action, CONTEXT);
      expect(problemOf(spec, { prefix: "lib", repository: "", reference: "main" })?.field, action).toBe(
        "repository",
      );
    }
  });

  it("builds each request from what was typed, trimmed", () => {
    expect(subtreeRequest("add", { prefix: " lib ", repository: "origin", reference: "main", squash: false })).toEqual({
      kind: "add",
      prefix: "lib",
      repository: "origin",
      reference: "main",
      squash: false,
    });
    expect(subtreeRequest("split", { prefix: "lib", branch: "lib-only", rejoin: true })).toEqual({
      kind: "split",
      prefix: "lib",
      branch: "lib-only",
      rejoin: true,
    });
    expect(subtreeRequest("reset", { prefix: "lib", reference: "abc123" })).toEqual({
      kind: "reset",
      prefix: "lib",
      reference: "abc123",
    });
    expect(subtreeRequest("push", { prefix: "lib", repository: "upstream", reference: "main" })).toEqual({
      kind: "push",
      prefix: "lib",
      repository: "upstream",
      reference: "main",
    });
  });

  it("warns before a reset, which replaces the folder", () => {
    expect(subtreeDialog("reset", CONTEXT).destructive).toBe(true);
    expect(subtreeDialog("split", CONTEXT).destructive).toBe(false);
  });
});

describe("lfs dialogs", () => {
  it("starts Track from the suggested pattern and sends it trimmed", () => {
    expect(lfsTrackDialog("*.psd").values.pattern).toBe("*.psd");
    expect(lfsTrackRequest({ pattern: " *.bin " })).toEqual({ kind: "track", pattern: "*.bin" });
    expect(problemOf(lfsTrackDialog(""), { pattern: "" })?.field).toBe("pattern");
  });

  it("confirms a prune as destructive", () => {
    expect(lfsPruneDialog().destructive).toBe(true);
  });
});

describe("the missing-lfs hint", () => {
  it("says so when a second look still finds nothing", () => {
    expect(lfsMissingDialog().intro.startsWith("Git has no lfs")).toBe(true);
    expect(lfsMissingDialog(true).intro.startsWith("Still not found.")).toBe(true);
    expect(lfsMissingDialog().confirm).toBe("Check Again");
  });
});

describe("follow-up", () => {
  it("names the folder after the URL until the user types one", () => {
    const values = { url: "https://h/team/library.git", path: "", branch: "" };
    expect(followUp(values, "url", new Set()).path).toBe("library");
    expect(followUp(values, "url", new Set(["path"])).path).toBe("");
    expect(followUp(values, "branch", new Set())).toBe(values);
  });
});
