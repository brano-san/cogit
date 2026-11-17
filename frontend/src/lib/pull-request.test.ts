import { describe, expect, it } from "vitest";
import { forgeOverride, forgeSetting, needsPush, parseRemote, pullRequestFor, pullRequestUrl } from "./pull-request";

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
      reason: "No GitHub, GitLab, Bitbucket or Azure DevOps remote",
    });
    expect(pullRequestFor({ remoteUrl: null, branch: feature, title: null })).toEqual({
      reason: "No GitHub, GitLab, Bitbucket or Azure DevOps remote",
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

  it("asks the fork a push goes to, not the upstream", () => {
    expect(needsPush({ name: "topic", upstream: "origin/topic", ahead: 3, pushAhead: 0 })).toBe(false);
    expect(needsPush({ name: "topic", upstream: "origin/topic", ahead: 0, pushAhead: 1 })).toBe(true);
  });
});

describe("parseRemote on Azure DevOps and Bitbucket Data Center", () => {
  const azure = "https://dev.azure.com/fabrikam/Fiber/_git/Web";
  const stash = "https://git.example.com/projects/PROJ/repos/web";

  it.each([
    ["https://dev.azure.com/fabrikam/Fiber/_git/Web", azure],
    ["https://fabrikam@dev.azure.com/fabrikam/Fiber/_git/Web", azure],
    ["https://dev.azure.com/fabrikam/Fiber/_git/Web/", azure],
    ["git@ssh.dev.azure.com:v3/fabrikam/Fiber/Web", azure],
    ["ssh://git@ssh.dev.azure.com/v3/fabrikam/Fiber/Web", azure],
    ["ssh://git@ssh.dev.azure.com:22/v3/fabrikam/Fiber/Web", azure],
    ["https://dev.azure.com/fabrikam/_git/Fiber", "https://dev.azure.com/fabrikam/_git/Fiber"],
    [
      "https://dev.azure.com/fabrikam/My%20Project/_git/My%20Repo",
      "https://dev.azure.com/fabrikam/My%20Project/_git/My%20Repo",
    ],
    [
      "git@ssh.dev.azure.com:v3/fabrikam/My%20Project/My%20Repo",
      "https://dev.azure.com/fabrikam/My%20Project/_git/My%20Repo",
    ],
    ["git@ssh.dev.azure.com:v3/fabrikam/My Project/Web", "https://dev.azure.com/fabrikam/My%20Project/_git/Web"],
    ["https://fabrikam.visualstudio.com/Fiber/_git/Web", "https://fabrikam.visualstudio.com/Fiber/_git/Web"],
    [
      "https://fabrikam.visualstudio.com/DefaultCollection/Fiber/_git/Web",
      "https://fabrikam.visualstudio.com/DefaultCollection/Fiber/_git/Web",
    ],
    ["https://fabrikam.visualstudio.com/_git/Fiber", "https://fabrikam.visualstudio.com/_git/Fiber"],
    ["fabrikam@vs-ssh.visualstudio.com:v3/fabrikam/Fiber/Web", "https://fabrikam.visualstudio.com/Fiber/_git/Web"],
    [
      "https://tfs.example.com:8080/tfs/DefaultCollection/Fiber/_git/Web",
      "https://tfs.example.com:8080/tfs/DefaultCollection/Fiber/_git/Web",
    ],
    [
      "ssh://tfs.example.com:22/tfs/DefaultCollection/Fiber/_git/Web",
      "https://tfs.example.com/tfs/DefaultCollection/Fiber/_git/Web",
    ],
  ])("reads %s as Azure DevOps", (url, base) => {
    expect(parseRemote(url)).toEqual({ host: "azure-devops", base });
  });

  it.each([
    ["https://git.example.com/scm/proj/web.git", stash],
    ["https://git.example.com/scm/proj/web", stash],
    ["https://jane@git.example.com/scm/proj/web.git", stash],
    ["https://git.example.com:8443/scm/proj/web.git", "https://git.example.com:8443/projects/PROJ/repos/web"],
    ["http://git.example.com:7990/scm/proj/web.git", "http://git.example.com:7990/projects/PROJ/repos/web"],
    ["https://git.example.com/bitbucket/scm/proj/web.git", "https://git.example.com/bitbucket/projects/PROJ/repos/web"],
    ["ssh://git@git.example.com:7999/proj/web.git", stash],
    ["ssh://git@git.example.com:7999/PROJ/web", stash],
    ["https://git.example.com/scm/~jane/notes.git", "https://git.example.com/users/jane/repos/notes"],
    ["ssh://git@git.example.com:7999/~jane/notes.git", "https://git.example.com/users/jane/repos/notes"],
    ["https://git.example.com/scm/proj/my%20repo.git", "https://git.example.com/projects/PROJ/repos/my%20repo"],
  ])("reads %s as Bitbucket Data Center", (url, base) => {
    expect(parseRemote(url)).toEqual({ host: "bitbucket-server", base });
  });

  it.each([
    "ssh://git@git.example.com:2222/proj/web.git",
    "ssh://git@git.example.com:7999/group/sub/web.git",
    "https://git.example.com/proj/web.git",
    "https://dev.azure.com/fabrikam/Fiber/Web",
    "git@ssh.dev.azure.com:v3/fabrikam/Fiber",
  ])("does not guess a service for %s", (url) => {
    expect(parseRemote(url)).toBeNull();
  });

  it("takes the service from the repository's setting for a server its URL does not tell", () => {
    expect(parseRemote("ssh://git@git.example.com:2222/proj/web.git", "bitbucket-server")).toEqual({
      host: "bitbucket-server",
      base: stash,
    });
    expect(parseRemote("https://git.example.com:8443/team/web.git", "gitlab")).toEqual({
      host: "gitlab",
      base: "https://git.example.com:8443/team/web",
    });
    expect(parseRemote("git@ghe.example.com:team/web.git", "github")).toEqual({
      host: "github",
      base: "https://ghe.example.com/team/web",
    });
    expect(parseRemote("https://git.example.com/scm/proj/web.git", "gitlab")?.host).toBe("gitlab");
  });

  it("returns nothing when the setting names a service whose URLs look otherwise", () => {
    expect(parseRemote("https://git.example.com/team/web.git", "azure-devops")).toBeNull();
    expect(parseRemote("https://git.example.com/team/sub/web.git", "bitbucket-server")).toBeNull();
  });
});

describe("forgeOverride", () => {
  it.each([
    [null, null],
    ["", null],
    ["auto", null],
    ["nonsense", null],
    ["azure-devops", "azure-devops"],
    [" Bitbucket-Server ", "bitbucket-server"],
  ])("reads %s as %s", (value, forge) => {
    expect(forgeOverride(value)).toBe(forge);
  });
});

describe("forgeSetting", () => {
  const entry = (local: string | null, inherited: string | null) => [
    { key: "cogit.hostingProvider", local, inherited },
  ];

  it("reads the repository's value over the inherited one", () => {
    expect(forgeSetting(entry("azure-devops", "gitlab"))).toBe("azure-devops");
    expect(forgeSetting(entry(null, "gitlab"))).toBe("gitlab");
    expect(forgeSetting(entry("auto", "gitlab"))).toBeNull();
    expect(forgeSetting([])).toBeNull();
  });
});

describe("pullRequestUrl on Azure DevOps and Bitbucket Data Center", () => {
  it.each([
    [
      "https://dev.azure.com/fabrikam/Fiber/_git/Web",
      "main",
      "feature/auth",
      "https://dev.azure.com/fabrikam/Fiber/_git/Web/pullrequestcreate?sourceRef=feature%2Fauth&targetRef=main",
    ],
    [
      "git@ssh.dev.azure.com:v3/fabrikam/My%20Project/Web",
      null,
      "topic",
      "https://dev.azure.com/fabrikam/My%20Project/_git/Web/pullrequestcreate?sourceRef=topic",
    ],
    [
      "https://git.example.com/scm/proj/web.git",
      "main",
      "feature/auth",
      "https://git.example.com/projects/PROJ/repos/web/pull-requests?create&sourceBranch=refs%2Fheads%2Ffeature%2Fauth&targetBranch=refs%2Fheads%2Fmain",
    ],
    [
      "ssh://git@git.example.com:7999/proj/web.git",
      null,
      "topic",
      "https://git.example.com/projects/PROJ/repos/web/pull-requests?create&sourceBranch=refs%2Fheads%2Ftopic",
    ],
  ])("builds the form for %s into %s from %s", (remote, base, head, url) => {
    expect(pullRequestUrl(remote, base, head, "Add the thing")).toBe(url);
  });

  it("opens the form for a server the setting names", () => {
    const plan = pullRequestFor({
      remoteUrl: "ssh://git@git.example.com:2222/proj/web.git",
      branch: { name: "topic", upstream: "origin/topic", ahead: 0 },
      title: "x",
      host: "bitbucket-server",
    });
    expect(plan).toEqual({
      url: "https://git.example.com/projects/PROJ/repos/web/pull-requests?create&sourceBranch=refs%2Fheads%2Ftopic",
    });
  });
});
