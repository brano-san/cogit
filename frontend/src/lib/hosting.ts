import type { HostingAccount, HostingProvider, NewHostingAccount } from "$lib/ipc";

export const PROVIDERS: readonly { id: HostingProvider; label: string }[] = [
  { id: "github", label: "GitHub" },
  { id: "gitlab", label: "GitLab" },
  { id: "bitbucket", label: "Bitbucket" },
];

export function providerLabel(provider: HostingProvider): string {
  return PROVIDERS.find((entry) => entry.id === provider)?.label ?? provider;
}

const CLOUD: Record<HostingProvider, string> = {
  github: "github.com",
  gitlab: "gitlab.com",
  bitbucket: "bitbucket.org",
};

export function selfHosted(provider: HostingProvider): boolean {
  return provider !== "bitbucket";
}

export function serverPlaceholder(provider: HostingProvider): string {
  return provider === "github"
    ? `Empty for ${CLOUD.github}; GitHub Enterprise: https://github.example.com`
    : `Empty for ${CLOUD.gitlab}; your own: https://gitlab.example.com`;
}

export function tokenHint(provider: HostingProvider): string {
  switch (provider) {
    case "github":
      return "A personal access token: GitHub ▸ Settings ▸ Developer settings ▸ Personal access tokens. Classic scopes: repo, read:org, read:user, workflow.";
    case "gitlab":
      return "A personal access token: GitLab ▸ Preferences ▸ Access tokens, with the scopes api, read_user and write_repository.";
    case "bitbucket":
      return "An API token with Bitbucket scopes: Atlassian account ▸ Security ▸ API tokens. Bitbucket signs in with the account's email and the token.";
  }
}

export function serverOf(account: HostingAccount): string {
  return account.serverUrl ?? CLOUD[account.provider];
}

export interface AccountDraft {
  provider: HostingProvider;
  server: string;
  email: string;
  token: string;
  useForGit: boolean;
}

export function draftProblem(draft: AccountDraft): string | null {
  const server = draft.server.trim();
  if (selfHosted(draft.provider) && server !== "" && /^http:\/\//i.test(server) && !/^http:\/\/(127\.0\.0\.1|localhost|\[::1\])([:/]|$)/i.test(server)) {
    return "The token is not sent over plain http://: use https://";
  }
  if (draft.provider === "bitbucket" && draft.email.trim() === "") return "Enter the email address of your Atlassian account";
  if (draft.token.trim() === "") return "Enter the access token";
  return null;
}

export function newAccount(draft: AccountDraft): NewHostingAccount {
  const server = draft.server.trim();
  return {
    provider: draft.provider,
    serverUrl: selfHosted(draft.provider) && server !== "" ? server : null,
    username: draft.provider === "bitbucket" ? draft.email.trim() : null,
    token: draft.token.trim(),
    useForGit: draft.useForGit,
  };
}
