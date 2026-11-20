import { describe, expect, it } from "vitest";
import { parseReview, reviewTitle, reviewUrl } from "./review-params";

describe("the review window's address", () => {
  it("survives a branch and a base with any character", () => {
    const request = { repo: 3, repoName: "my repo", branch: "feature/a+b&c", base: "origin/main" };
    const url = reviewUrl(request);
    expect(url.startsWith("review.html?")).toBe(true);
    expect(parseReview(url.slice(url.indexOf("?")))).toEqual(request);
  });

  it("leaves the base out until one is chosen", () => {
    const url = reviewUrl({ repo: 1, repoName: "", branch: "dev", base: null });
    expect(url).not.toContain("base=");
    expect(parseReview(url.slice(url.indexOf("?")))?.base).toBeNull();
  });

  it("refuses an address without a repository or a branch", () => {
    expect(parseReview("?repo=1")).toBeNull();
    expect(parseReview("?branch=dev")).toBeNull();
  });

  it("titles the window with the branch and the repository", () => {
    expect(reviewTitle("dev", "cogit")).toBe("dev [cogit] - Review Branch");
    expect(reviewTitle("dev", "")).toBe("dev - Review Branch");
  });
});
