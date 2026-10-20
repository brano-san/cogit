import type { AuthRequired, GitError, SignIn } from "$lib/ipc/bindings";

/** What the Sign in dialog is opened with: the host the remote refused, the username tried
    there, and git's words when what was typed did not work either. */
export interface SignInRequest {
  host: string;
  username: string | null;
  error: string | null;
}

export type AskSignIn = (request: SignInRequest) => Promise<SignIn | null>;

/** The backend recognizes a refusal and names its host (F-082); nothing here reads git's words. */
export function authRequiredOf(err: unknown): AuthRequired | null {
  const detail = (err as { detail?: GitError } | null)?.detail;
  return detail?.kind === "command" ? (detail.data?.authRequired ?? null) : null;
}

function gitText(err: unknown): string | null {
  const detail = (err as { detail?: GitError } | null)?.detail;
  if (detail?.kind !== "command") return null;
  return detail.data.stderr.trim() || detail.data.stdout.trim() || null;
}

/** Runs `operation`; a refusal opens the Sign in dialog and runs it again with what was typed,
    until it works or the dialog is canceled, which gives back the command's own error. */
export async function signedIn<T>(operation: (credential: SignIn | null) => Promise<T>, ask: AskSignIn): Promise<T> {
  let credential: SignIn | null = null;
  for (;;) {
    try {
      return await operation(credential);
    } catch (err) {
      const refused = authRequiredOf(err);
      if (refused === null) throw err;
      const answer = await ask({
        host: refused.host,
        username: refused.username ?? credential?.username ?? null,
        error: credential === null ? null : gitText(err),
      });
      if (answer === null) throw err;
      credential = answer;
    }
  }
}
