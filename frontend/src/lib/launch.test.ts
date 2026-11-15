import { describe, expect, it, vi } from "vitest";
import type { DropHost } from "$lib/drop-open";
import type { LaunchRequest } from "$lib/ipc";
import { runLaunch } from "./launch";

function host() {
  const steps: string[] = [];
  const fake: DropHost = {
    openInList: vi.fn(async (path: string) => void steps.push(`list ${path}`)),
    activate: vi.fn(async (path: string) => void steps.push(`show ${path}`)),
    refreshList: vi.fn(async () => void steps.push("refresh")),
    report: vi.fn(),
  };
  return { fake, steps };
}

function request(show: LaunchRequest["show"], ...paths: [string, string][]): LaunchRequest {
  return { show, paths: paths.map(([path, folder]) => ({ path, folder })) };
}

describe("runLaunch", () => {
  it("opens bare paths the way folders dropped on the window open, a file as its folder", async () => {
    const { fake, steps } = host();
    await runLaunch(request("repository", ["D:/a", "D:/a"], ["D:/b/f.txt", "D:/b"], ["D:/a/", "D:/a/"]), fake);
    expect(steps).toEqual(["list D:/b", "refresh", "show D:/a"]);
  });
});
