/** How a network command's credential step ended, read off git's own words. The one place
    that knows them: later, accounts stored in Cogit plug in before this (supplying the
    credentials, so no helper dialog is needed) and this stays the fallback. */
export type CredentialTrouble = "canceled" | "noPrompt";

/** Git Credential Manager's words when its sign-in window is closed; git's "could not read
    Username … terminal prompts disabled" follows them and must not count as no helper. */
const CANCELED = /user cancell?ed (the )?dialog|cancell?ed by the user|authentication (was )?cancell?ed|sign-?in (was )?cancell?ed/i;
/** No helper could ask: git wanted to prompt on a terminal, and Cogit runs it without one. */
const NO_PROMPT = /terminal prompts disabled|could not read (username|password)|cannot prompt because/i;

export function credentialTrouble(output: string): CredentialTrouble | null {
  if (CANCELED.test(output)) return "canceled";
  if (NO_PROMPT.test(output)) return "noPrompt";
  return null;
}

/** What the Errors window adds above the raw output when no credentials could be asked for. */
export const NO_PROMPT_ADVICE =
  "Git needed a user name and password for this remote, and nothing could ask for them: no credential helper is configured, " +
  "or the one configured cannot show a window. Install Git Credential Manager (it comes with Git for Windows) or set one " +
  "with `git config --global credential.helper manager`; or switch the remote to an SSH URL (Remotes ▸ Edit URL…).";
