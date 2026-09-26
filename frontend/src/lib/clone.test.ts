import { describe, expect, it, vi } from "vitest";
import {
  branchOptions,
  cloneRequest,
  destinationToAsk,
  directoryProblem,
  initialBranch,
  isUrl,
  joinPath,
  limitProblem,
  parentFolder,
  runClone,
  sourceProblem,
  type CloneChoices,
} from "./clone";

describe("the repository to clone", () => {
  it("takes a URL in any of git's forms", () => {
    for (const url of [
      "https://github.com/owner/app.git",
      "ssh://git@host:2222/app.git",
      "git@github.com:owner/app.git",
      "github.com:owner/app",
      "file:///D:/repos/app",
    ]) {
      expect(isUrl(url), url).toBe(true);
      expect(sourceProblem(url), url).toBeNull();
    }
  });

  it("takes a folder named in full, and nothing named by halves", () => {
    expect(isUrl("C:\\repos\\app")).toBe(false);
    expect(sourceProblem("C:\\repos\\app")).toBeNull();
    expect(sourceProblem("\\\\server\\share\\app")).toBeNull();
    expect(sourceProblem("/home/me/app")).toBeNull();
    expect(sourceProblem("repos/app")).toBe("Enter a URL, or the full path of a folder");
  });

  it("says what is missing", () => {
    expect(sourceProblem("   ")).toBe("Enter the URL or the folder of a repository");
    expect(sourceProblem("--upload-pack=x")).toBe("A repository cannot start with “-”");
  });
});

describe("the size limit of a partial clone", () => {
  it("is whole megabytes from one, and asked only when ticked", () => {
    expect(limitProblem(false, "")).toBeNull();
    expect(limitProblem(true, " 5 ")).toBeNull();
    for (const bad of ["", "0", "1.5", "-2", "a"]) expect(limitProblem(true, bad), bad).not.toBeNull();
  });
});

describe("the destination", () => {
  it("joins in the parent's own separator", () => {
    expect(joinPath("D:\\src", "app")).toBe("D:\\src\\app");
    expect(joinPath("D:/src/", "app")).toBe("D:/src/app");
    expect(joinPath("D:\\", "app")).toBe("D:\\app");
    expect(joinPath("/home/me", "app")).toBe("/home/me/app");
  });

  it("goes beside the repository in front by default", () => {
    expect(parentFolder("D:/src/cogit")).toBe("D:/src");
    expect(parentFolder("D:\\src\\cogit\\")).toBe("D:\\src");
    expect(parentFolder("D:\\cogit")).toBe("D:\\");
    expect(parentFolder("/repo")).toBe("/");
    expect(parentFolder("")).toBe("");
  });

  const at = (kind: "missing" | "empty" | "notEmpty" | "file" | "unreadable") => ({
    parent: "D:\\src",
    name: "app",
    seen: { path: "D:\\src\\app", kind },
  });

  it("allows a missing or empty folder only", () => {
    expect(directoryProblem(at("missing"))).toBeNull();
    expect(directoryProblem(at("empty"))).toBeNull();
    expect(directoryProblem(at("notEmpty"))).toBe("D:\\src\\app is not empty");
    expect(directoryProblem(at("file"))).toBe("D:\\src\\app is a file");
  });

  it("waits for the answer about the path now in the fields", () => {
    expect(directoryProblem({ ...at("empty"), name: "other" })).toBe("Checking the folder…");
    expect(destinationToAsk("D:\\src", " other ")).toBe("D:\\src\\other");
  });

  it("asks nothing of a name that cannot be a folder", () => {
    expect(directoryProblem({ parent: "", name: "app", seen: null })).toBe("Choose the folder to clone into");
    expect(directoryProblem({ parent: "src", name: "app", seen: null })).toBe(
      "Enter the full path of the parent folder",
    );
    expect(directoryProblem({ parent: "D:\\src", name: "a:b", seen: null })).toMatch(/cannot contain/);
    expect(destinationToAsk("D:\\src", "")).toBeNull();
  });
});

describe("the branch to check out", () => {
  const listing = { defaultBranch: "main", branches: ["dev", "feature/x", "main"] };

  it("lists the server's default first and marked", () => {
    expect(branchOptions(listing)).toEqual([
      ["main", "main (default)"],
      ["dev", "dev"],
      ["feature/x", "feature/x"],
    ]);
    expect(initialBranch(listing)).toBe("main");
  });

  it("starts from the first branch when HEAD names none of them", () => {
    const unborn = { defaultBranch: "trunk", branches: ["dev"] };
    expect(branchOptions(unborn)).toEqual([["dev", "dev"]]);
    expect(initialBranch(unborn)).toBe("dev");
    expect(initialBranch({ defaultBranch: "main", branches: [] })).toBeNull();
  });
});

describe("the request Finish sends", () => {
  const choices: CloneChoices = {
    source: " https://host/app.git ",
    submodules: true,
    allBranches: true,
    branch: "main",
    listing: { defaultBranch: "main", branches: ["dev", "main"] },
    skipLarge: false,
    limitMb: "1",
    parent: "D:\\src",
    name: " app ",
  };

  it("names a branch only when it is not the server's default", () => {
    expect(cloneRequest(choices)).toEqual({
      source: "https://host/app.git",
      target: "D:\\src\\app",
      submodules: true,
      allBranches: true,
      branch: null,
      skipLargerThanMb: null,
    });
    expect(cloneRequest({ ...choices, branch: "dev" }).branch).toBe("dev");
    expect(cloneRequest({ ...choices, branch: "dev", listing: null }).branch).toBeNull();
  });

  it("carries the size limit only when files are skipped", () => {
    expect(cloneRequest({ ...choices, skipLarge: true, limitMb: " 8 " }).skipLargerThanMb).toBe(8);
  });
});

describe("running the clone", () => {
  const request = cloneRequest({
    source: "https://host/app.git",
    submodules: true,
    allBranches: true,
    branch: null,
    listing: null,
    skipLarge: false,
    limitMb: "1",
    parent: "D:\\src",
    name: "app",
  });

  it("opens what it cloned", async () => {
    const open = vi.fn(async () => {});
    const deps = {
      run: (operation: (onLine: (line: string) => void) => Promise<string>) => operation(() => {}),
      clone: vi.fn(async () => "D:\\src\\app"),
      open,
      report: vi.fn(),
    };

    expect(await runClone(request, deps)).toBe("D:\\src\\app");
    expect(deps.clone).toHaveBeenCalledWith(request, expect.any(Function));
    expect(open).toHaveBeenCalledWith("D:\\src\\app");
    expect(deps.report).not.toHaveBeenCalled();
  });

  it("reports a failure, a cancel included, and opens nothing", async () => {
    const failure = new Error("fatal: repository not found");
    const deps = {
      run: (operation: (onLine: (line: string) => void) => Promise<string>) => operation(() => {}),
      clone: vi.fn(async () => Promise.reject(failure)),
      open: vi.fn(async () => {}),
      report: vi.fn(),
    };

    expect(await runClone(request, deps)).toBeNull();
    expect(deps.report).toHaveBeenCalledWith(failure, "Could not clone the repository");
    expect(deps.open).not.toHaveBeenCalled();
  });
});
