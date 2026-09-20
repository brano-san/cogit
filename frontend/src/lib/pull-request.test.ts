import { describe, expect, it } from "vitest";
import { authHost, parseRemote, pullRequestUrl } from "./pull-request";

describe("parseRemote", () => {
  it("reads an ssh GitHub remote", () => {
    expect(parseRemote("git@github.com:brano-san/cogit.git")).toEqual({
      host: "github",
      base: "https://github.com/brano-san/cogit",
    });
  });

  it("reads an https GitHub remote", () => {
    expect(parseRemote("https://github.com/brano-san/cogit.git")?.host).toBe("github");
  });

  it("keeps a nested GitLab group in the path", () => {
    expect(parseRemote("git@gitlab.com:team/sub/project.git")?.base).toBe(
      "https://gitlab.com/team/sub/project",
    );
  });

  it("recognises Bitbucket", () => {
    expect(parseRemote("https://bitbucket.org/team/repo.git")?.host).toBe("bitbucket");
  });

  it("returns nothing for a host it does not know", () => {
    expect(parseRemote("git@git.internal.example:team/repo.git")).toBeNull();
  });

  it("returns nothing for a local path", () => {
    expect(parseRemote("C:/repos/origin.git")).toBeNull();
  });

  it("works without the .git suffix", () => {
    expect(parseRemote("https://github.com/a/b")?.base).toBe("https://github.com/a/b");
  });
});

describe("pullRequestUrl", () => {
  const github = "git@github.com:a/b.git";

  it("builds the GitHub compare form with both branches", () => {
    const url = pullRequestUrl(github, "main", "topic", "Add the thing");

    expect(url).toContain("https://github.com/a/b/compare/main...topic");
    expect(url).toContain("expand=1");
  });

  it("puts the commit subject in the title", () => {
    const url = pullRequestUrl(github, "main", "topic", "Add the thing");
    expect(url).toContain("title=Add+the+thing");
  });

  it("escapes a branch name with a slash", () => {
    const url = pullRequestUrl(github, "main", "feature/auth", "x");
    expect(url).toContain("compare/main...feature%2Fauth");
  });

  it("builds the GitLab merge request form", () => {
    const url = pullRequestUrl("git@gitlab.com:a/b.git", "main", "topic", "x");

    expect(url).toContain("/-/merge_requests/new");
    expect(url).toContain("merge_request%5Bsource_branch%5D=topic");
  });

  it("builds the Bitbucket form", () => {
    const url = pullRequestUrl("https://bitbucket.org/a/b.git", "main", "topic", "x");

    expect(url).toContain("/pull-requests/new");
    expect(url).toContain("source=topic");
    expect(url).toContain("dest=main");
  });

  it("returns nothing for an unknown host so the button can stay hidden", () => {
    expect(pullRequestUrl("git@internal:a/b.git", "main", "topic", "x")).toBeNull();
  });

  it("returns nothing when the branch would compare against itself", () => {
    expect(pullRequestUrl(github, "main", "main", "x")).toBeNull();
  });
});

describe("authHost", () => {
  it("names the host of an HTTPS remote", () => {
    expect(authHost("https://github.com/owner/repo.git")).toBe("github.com");
  });

  it("drops a user embedded in the URL", () => {
    expect(authHost("https://me@gitlab.com/o/r.git")).toBe("gitlab.com");
  });

  it("drops a port", () => {
    expect(authHost("https://git.example.com:8443/o/r.git")).toBe("git.example.com");
  });

  it("has no host for SSH, which authenticates through the agent", () => {
    expect(authHost("git@github.com:owner/repo.git")).toBeNull();
    expect(authHost("ssh://git@github.com/o/r.git")).toBeNull();
  });

  it("has no host for a local path or for nothing", () => {
    expect(authHost("/srv/git/repo.git")).toBeNull();
    expect(authHost(null)).toBeNull();
  });
});
