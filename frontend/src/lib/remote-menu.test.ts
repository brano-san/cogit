import { describe, expect, it, vi } from "vitest";
import type { Submodule } from "./ipc";
import {
  LFS_UNKNOWN,
  NO_FILE,
  NO_LFS,
  NO_REPOSITORY,
  NO_SUBMODULES,
  UNCOMMITTED,
  remoteCommands,
  submoduleScope,
  trackSuggestion,
  type RemoteMenuActions,
  type RemoteMenuContext,
} from "./remote-menu";

const OPEN: RemoteMenuContext = {
  repository: true,
  remote: true,
  changes: false,
  submodules: 2,
  lfs: "git-lfs/3.7.0",
  files: ["art/cover.psd"],
};

function actions(): RemoteMenuActions {
  return {
    synchronize: vi.fn(),
    submodule: vi.fn(),
    subtree: vi.fn(),
    lfs: vi.fn(),
    repoSettings: vi.fn(),
  };
}

function reasons(context: RemoteMenuContext): Record<string, string | undefined> {
  return Object.fromEntries(remoteCommands(context, actions()).map((c) => [c.id, c.unavailable]));
}

const MENU_IDS = [
  "synchronize",
  "submodule-init",
  "submodule-sync",
  "submodule-reset",
  "submodule-add",
  "submodule-deactivate",
  "submodule-deinit",
  "submodule-unregister",
  "subtree-add",
  "subtree-merge",
  "subtree-split",
  "subtree-reset",
  "subtree-push",
  "lfs-install",
  "lfs-track",
  "lfs-lock",
  "lfs-unlock",
  "lfs-prune",
  "repo-settings",
];

describe("remote commands", () => {
  it("has one command per native menu item", () => {
    expect(remoteCommands(OPEN, actions()).map((c) => c.id)).toEqual(MENU_IDS);
  });

  it("offers everything in a repository that has what each needs", () => {
    expect(Object.values(reasons(OPEN)).filter(Boolean)).toEqual([]);
  });

  it("disables every one without a repository, rather than hiding it", () => {
    const closed = reasons({ ...OPEN, repository: false });
    for (const id of MENU_IDS) expect(closed[id], id).toBe(NO_REPOSITORY);
  });

  it("offers the submodule commands only where there are submodules, except Add", () => {
    const none = reasons({ ...OPEN, submodules: 0 });
    expect(none["submodule-add"]).toBeUndefined();
    for (const id of ["submodule-init", "submodule-sync", "submodule-reset", "submodule-deinit"]) {
      expect(none[id], id).toBe(NO_SUBMODULES);
    }
  });

  it("keeps subtree add and merge off a dirty tree, which git subtree refuses", () => {
    const dirty = reasons({ ...OPEN, changes: true });
    expect(dirty["subtree-add"]).toBe(UNCOMMITTED);
    expect(dirty["subtree-merge"]).toBe(UNCOMMITTED);
    expect(dirty["subtree-split"]).toBeUndefined();
    expect(dirty["subtree-push"]).toBeUndefined();
  });

  it("leaves only Install without git lfs", () => {
    const missing = reasons({ ...OPEN, lfs: null });
    expect(missing["lfs-install"]).toBeUndefined();
    for (const id of ["lfs-track", "lfs-lock", "lfs-unlock", "lfs-prune"]) expect(missing[id], id).toBe(NO_LFS);
  });

  it("says it is still looking while the answer is not in", () => {
    expect(reasons({ ...OPEN, lfs: undefined })["lfs-track"]).toBe(LFS_UNKNOWN);
  });

  it("needs a file to lock or unlock", () => {
    const nothing = reasons({ ...OPEN, files: [] });
    expect(nothing["lfs-lock"]).toBe(NO_FILE);
    expect(nothing["lfs-unlock"]).toBe(NO_FILE);
    expect(nothing["lfs-track"]).toBeUndefined();
  });

  it("synchronize needs a remote", () => {
    expect(reasons({ ...OPEN, remote: false }).synchronize).toBe("This repository has no remote");
  });

  it("runs the action the item stands for", () => {
    const wired = actions();
    const commands = remoteCommands(OPEN, wired);
    commands.find((c) => c.id === "submodule-deinit")?.run();
    commands.find((c) => c.id === "subtree-split")?.run();
    commands.find((c) => c.id === "lfs-prune")?.run();
    commands.find((c) => c.id === "repo-settings")?.run();
    expect(wired.submodule).toHaveBeenCalledWith("deinit");
    expect(wired.subtree).toHaveBeenCalledWith("split");
    expect(wired.lfs).toHaveBeenCalledWith("prune");
    expect(wired.repoSettings).toHaveBeenCalled();
  });
});

function module(path: string): Submodule {
  return {
    name: path,
    path,
    url: "",
    recorded: "",
    checkedOut: null,
    state: "inSync",
    branch: null,
    subject: null,
    nested: false,
    ahead: 0,
    behind: 0,
    repoState: null,
  };
}

describe("submodule scope", () => {
  const children = new Map([
    ["", [module("vendor/lib"), module("vendor/middle")]],
    ["vendor/middle", [module("deep/inner")]],
  ]);

  it("is the top repository's submodules when none is open", () => {
    expect(submoduleScope({ children, open: null, selected: [] })).toEqual({
      parent: "",
      choices: ["vendor/lib", "vendor/middle"],
      path: null,
    });
  });

  it("points at a submodule picked in the Files panel", () => {
    const scope = submoduleScope({ children, open: null, selected: ["README.md", "vendor/lib"] });
    expect(scope.path).toBe("vendor/lib");
  });

  it("acts on an opened submodule in the repository that holds it", () => {
    expect(submoduleScope({ children, open: "vendor/middle/deep/inner", selected: [] })).toEqual({
      parent: "vendor/middle",
      choices: ["deep/inner"],
      path: "deep/inner",
    });
    expect(submoduleScope({ children, open: "vendor/lib", selected: [] }).path).toBe("vendor/lib");
  });
});

describe("track suggestion", () => {
  it("is the extension of the file at hand", () => {
    expect(trackSuggestion(["art/cover.psd"])).toBe("*.psd");
    expect(trackSuggestion(["Makefile"])).toBe("");
    expect(trackSuggestion([".gitignore"])).toBe("");
    expect(trackSuggestion([])).toBe("");
  });
});

// Remote ▸ Synchronise in a detached HEAD ran and failed: "You are not currently on a branch".
describe("Synchronise and the branch", () => {
  it("says what the toolbar's Sync says once there is a remote", () => {
    expect(reasons({ ...OPEN, syncBlocked: "HEAD is not on a branch" }).synchronize).toBe("HEAD is not on a branch");
    expect(reasons({ ...OPEN, remote: false, syncBlocked: "HEAD is not on a branch" }).synchronize).toBe(
      "This repository has no remote",
    );
    expect(reasons(OPEN).synchronize).toBeUndefined();
  });
});
