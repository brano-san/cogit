/** What the Errors window adds above the raw output when no credentials could be asked for:
    the Sign in dialog was canceled, or the command is one that does not offer it (F-082).
    Whether that is the case the backend reads off git's words (`GitOutput.credentialTrouble`). */
export const NO_PROMPT_ADVICE =
  "Git needed a user name and password for this remote, and nothing could ask for them. Run the command again and " +
  "sign in when Cogit asks, or add the host in Preferences ▸ Authentication; or set up a credential helper such as " +
  "Git Credential Manager (`git config --global credential.helper manager`); or switch the remote to an SSH URL " +
  "(Remotes ▸ Edit URL…).";
