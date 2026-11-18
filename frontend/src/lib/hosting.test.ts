import { describe, expect, it } from "vitest";
import { draftProblem, newAccount, serverOf, type AccountDraft } from "./hosting";

const draft = (patch: Partial<AccountDraft>): AccountDraft => ({
  provider: "github",
  server: "",
  email: "",
  token: "ghp_x",
  useForGit: true,
  ...patch,
});

describe("hosting account drafts", () => {
  it("needs a token, and an email for Bitbucket", () => {
    expect(draftProblem(draft({}))).toBeNull();
    expect(draftProblem(draft({ token: "  " }))).toMatch(/token/);
    expect(draftProblem(draft({ provider: "bitbucket" }))).toMatch(/email/);
    expect(draftProblem(draft({ provider: "bitbucket", email: "a@b.io" }))).toBeNull();
  });

  it("refuses plain http except to this machine", () => {
    expect(draftProblem(draft({ server: "http://ghe.corp" }))).toMatch(/https/);
    expect(draftProblem(draft({ server: "http://127.0.0.1:8080" }))).toBeNull();
    expect(draftProblem(draft({ server: "ghe.corp" }))).toBeNull();
  });

  it("sends a server only where one can be self-hosted, an email only for Bitbucket", () => {
    expect(newAccount(draft({ server: " https://ghe.corp ", email: "x" }))).toEqual({
      provider: "github",
      serverUrl: "https://ghe.corp",
      username: null,
      token: "ghp_x",
      useForGit: true,
    });
    expect(newAccount(draft({ provider: "bitbucket", server: "https://x", email: " a@b.io " }))).toMatchObject({
      serverUrl: null,
      username: "a@b.io",
    });
  });

  it("names the cloud server of an account without one", () => {
    const account = { id: "1", provider: "gitlab" as const, serverUrl: null, login: "ann", useForGit: true };
    expect(serverOf(account)).toBe("gitlab.com");
    expect(serverOf({ ...account, serverUrl: "https://corp/gitlab" })).toBe("https://corp/gitlab");
  });
});
