import { describe, expect, it } from "vitest";
import { authHost, needsPush, parseRemote, pullRequestFor, pullRequestUrl } from "./pull-request";

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

  // The ssh:// form, with or without a port, found no match: no link to a pull request.
  it("reads the ssh:// form of a remote too", () => {
    expect(parseRemote("ssh://git@github.com/o/r.git")).toEqual({ host: "github", base: "https://github.com/o/r" });
    expect(parseRemote("ssh://git@github.com:22/o/r.git")).toEqual({ host: "github", base: "https://github.com/o/r" });
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

// base came from the branch's own upstream: feature tracking origin/feature compared with
// itself, and Create Pull Request went off with "No GitHub, GitLab or Bitbucket remote".
describe("pullRequestFor", () => {
  const github = "git@github.com:a/b.git";
  const feature = { name: "feature", upstream: "origin/feature", ahead: 0 };

  it("opens the form for a pushed branch, against the forge's default branch", () => {
    const plan = pullRequestFor({ remoteUrl: github, branch: feature, title: "Add the thing" });

    expect("url" in plan && plan.url).toContain("https://github.com/a/b/compare/feature?");
    expect("url" in plan && plan.url).toContain("title=Add+the+thing");
  });

  it("leaves the target to GitLab and Bitbucket too", () => {
    const lab = pullRequestFor({ remoteUrl: "git@gitlab.com:a/b.git", branch: feature, title: "x" });
    const bucket = pullRequestFor({ remoteUrl: "https://bitbucket.org/a/b.git", branch: feature, title: "x" });

    expect("url" in lab && lab.url).not.toContain("target_branch");
    expect("url" in bucket && bucket.url).not.toContain("dest=");
  });

  it("titles the form with the branch name when there is no subject", () => {
    const plan = pullRequestFor({ remoteUrl: github, branch: feature, title: null });
    expect("url" in plan && plan.url).toContain("title=feature");
  });

  it("says why when HEAD is on no branch, or the remote is no forge", () => {
    expect(pullRequestFor({ remoteUrl: github, branch: undefined, title: null })).toEqual({
      reason: "HEAD is not on a branch",
    });
    expect(pullRequestFor({ remoteUrl: "git@internal:a/b.git", branch: feature, title: null })).toEqual({
      reason: "No GitHub, GitLab or Bitbucket remote",
    });
    expect(pullRequestFor({ remoteUrl: null, branch: feature, title: null })).toEqual({
      reason: "No GitHub, GitLab or Bitbucket remote",
    });
  });
});

// A branch never pushed has no upstream, and ahead is 0 then: the push was not offered and
// the browser opened a compare form for a branch the remote does not have.
describe("needsPush", () => {
  it("is true for a branch never pushed and for one with commits the remote lacks", () => {
    expect(needsPush({ name: "new", upstream: null, ahead: 0 })).toBe(true);
    expect(needsPush({ name: "topic", upstream: "origin/topic", ahead: 2 })).toBe(true);
  });

  it("is false for a branch the remote has in full", () => {
    expect(needsPush({ name: "topic", upstream: "origin/topic", ahead: 0 })).toBe(false);
  });
});
