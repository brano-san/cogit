import { describe, expect, it, vi } from "vitest";
import type { LaunchRequest, RepoId } from "$lib/ipc";
import { pathInRepo, runLaunch, type LaunchHost } from "./launch";

const REPO = 7 as RepoId;

function host(opened: string | null = "D:\\work\\repo") {
  const steps: string[] = [];
  const fake: LaunchHost = {
    openInList: vi.fn(async (path: string) => void steps.push(`list ${path}`)),
    activate: vi.fn(async (path: string) => {
      steps.push(`show ${path}`);
      return opened;
    }),
    refreshList: vi.fn(async () => void steps.push("refresh")),
    report: vi.fn((_err: unknown, title: string) => void steps.push(`error ${title}`)),
    shown: () => (opened === null ? null : REPO),
    investigate: vi.fn(async (repo: RepoId, path: string) => void steps.push(`investigate ${repo} ${path}`)),
    blame: vi.fn(async (repo: RepoId, path: string) => void steps.push(`blame ${repo} ${path}`)),
  };
  return { fake, steps };
}

function request(show: LaunchRequest["show"], ...paths: [string, string][]): LaunchRequest {
  return { show, paths: paths.map(([path, folder]) => ({ path, folder })) };
}

describe("pathInRepo", () => {
  it("names a file inside the repository relative to its root, with slashes", () => {
    expect(pathInRepo("D:\\work\\repo", "D:\\work\\repo\\src\\a.rs")).toBe("src/a.rs");
    expect(pathInRepo("/home/me/repo/", "/home/me/repo/a b.rs")).toBe("a b.rs");
  });

  it("compares a Windows path without case and keeps the file's own spelling", () => {
    expect(pathInRepo("D:/Work/Repo", "d:\\work\\repo\\Src\\A.rs")).toBe("Src/A.rs");
    expect(pathInRepo("//server/share/repo", "\\\\SERVER\\share\\repo\\x")).toBe("x");
  });

  it("refuses a path outside, a sibling with the same prefix and the root itself", () => {
    expect(pathInRepo("D:/work/repo", "D:/work/other/a.rs")).toBeNull();
    expect(pathInRepo("D:/work/repo", "D:/work/repo2/a.rs")).toBeNull();
    expect(pathInRepo("D:/work/repo", "D:/work/repo")).toBeNull();
    expect(pathInRepo("/home/me/Repo", "/home/me/repo/a")).toBeNull();
  });
});

describe("runLaunch", () => {
  it("opens bare paths the way folders dropped on the window open", async () => {
    const { fake, steps } = host();
    await runLaunch(request("repository", ["D:/a", "D:/a"], ["D:/b/f.txt", "D:/b"], ["D:/a/", "D:/a/"]), fake);
    expect(steps).toEqual(["list D:/b", "refresh", "show D:/a"]);
  });

  it("opens Investigate for a file's log, inside the repository that holds it", async () => {
    const { fake, steps } = host();
    await runLaunch(request("log", ["D:\\work\\repo\\src\\a.rs", "D:\\work\\repo\\src"]), fake);
    expect(steps).toEqual(["show D:\\work\\repo\\src", "investigate 7 src/a.rs"]);
  });

  it("shows a folder's log as the repository's graph", async () => {
    const { fake, steps } = host();
    await runLaunch(request("log", ["D:\\work\\repo\\src", "D:\\work\\repo\\src"]), fake);
    expect(steps).toEqual(["show D:\\work\\repo\\src"]);
  });

  it("opens Blame for a file and refuses a folder before opening anything", async () => {
    const { fake, steps } = host();
    await runLaunch(request("blame", ["D:\\work\\repo\\a.rs", "D:\\work\\repo"]), fake);
    await runLaunch(request("blame", ["D:\\work\\repo", "D:\\work\\repo"]), fake);
    expect(steps).toEqual(["show D:\\work\\repo", "blame 7 a.rs", "error Could not open Blame"]);
  });

  it("stops when the repository does not open: the open reported why", async () => {
    const { fake, steps } = host(null);
    await runLaunch(request("blame", ["D:\\nowhere\\a.rs", "D:\\nowhere"]), fake);
    expect(steps).toEqual(["show D:\\nowhere"]);
  });

  it("reports a window that fails to open", async () => {
    const { fake, steps } = host();
    fake.investigate = vi.fn(async () => {
      throw new Error("gone");
    });
    await runLaunch(request("log", ["D:\\work\\repo\\a.rs", "D:\\work\\repo"]), fake);
    expect(steps).toEqual(["show D:\\work\\repo", "error Could not open Investigate"]);
  });
});
