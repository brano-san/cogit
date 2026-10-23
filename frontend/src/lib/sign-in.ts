import type { AuthRequired, GitError, SignIn } from "$lib/ipc/bindings";
import { CogitError } from "$lib/ipc";

/** What the Sign in dialog is opened with: the host the remote refused, the username tried
    there, and git's words when what was typed did not work either. */
export interface SignInRequest {
  host: string;
  username: string | null;
  error: string | null;
  /** An ssh remote refused: the dialog opens on its SSH key tab. */
  ssh: boolean;
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
    until it works or the dialog is closed, which ends it as canceled: the user's choice, not an
    error, and nothing reaches the Errors window (its refusal was journaled as answered). */
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
        ssh: refused.ssh,
      });
      if (answer === null) {
        const detail = (err as { detail?: GitError } | null)?.detail;
        throw new CogitError({ kind: "cancelled", data: detail?.kind === "command" ? detail.data.command : "" });
      }
      credential = answer;
    }
  }
}

const NOT_KEYS = ["known_hosts", "known_hosts.old", "config", "authorized_keys", "authorized_keys2"];

/** What is wrong with `path` as an ssh private key, as far as its name tells; `null`: nothing. */
export function keyFileProblem(path: string): string | null {
  const name = path.trim().split(/[\\/]/).pop() ?? "";
  if (name.endsWith(".pub")) return "That is the public key: choose the file without .pub.";
  if (NOT_KEYS.includes(name)) return `${name} is no key: choose a private key file.`;
  return null;
}
