import {
  changeMasterPassword,
  createCredentialStore,
  credentialStore,
  resetCredentialStore,
  unlockCredentials,
} from "$lib/ipc";

/** `unlock`: the store is locked. `create`: the first Remember. `change`: Preferences.
    `forgot`: a new master password, and everything stored goes. */
export type MasterMode = "unlock" | "create" | "change" | "forgot";

/** `password` `null`: "Don't use a master password". */
export interface MasterAnswer {
  current: string;
  password: string | null;
}

type Answer = MasterAnswer | "forgot" | null;

interface Pending {
  mode: MasterMode;
  /** `change` asks for the current master password only when there is one. */
  hasCurrent: boolean;
  error: string | null;
  settle: (answer: Answer) => void;
}

/** Cogit's password store as the user meets it (R-891): the master password is asked once
    per session, on the first use of the store, and kept by the backend until Cogit exits. */
class MasterPasswordStore {
  open = $state.raw<Pending | null>(null);
  /** The unlock was canceled: not asked again before each command, only when needed. */
  #declined = false;

  #ask(mode: MasterMode, error: string | null = null, hasCurrent = true): Promise<Answer> {
    this.open?.settle(null);
    return new Promise((resolve) => {
      this.open = {
        mode,
        hasCurrent,
        error,
        settle: (answer) => {
          this.open = null;
          resolve(answer);
        },
      };
    });
  }

  answer(value: Answer): void {
    this.open?.settle(value);
  }

  /** Before a command that may use a stored sign-in. */
  async beforeUse(): Promise<void> {
    if (!this.#declined) await this.unlock();
  }

  /** `true`: the store is open, or there is none yet. */
  async unlock(): Promise<boolean> {
    if ((await credentialStore()).state !== "locked") return true;
    let error: string | null = null;
    for (;;) {
      const answer = await this.#ask("unlock", error);
      if (answer === null) {
        this.#declined = true;
        return false;
      }
      if (answer === "forgot") return await this.#forgot();
      if (await unlockCredentials(answer.current)) {
        this.#declined = false;
        return true;
      }
      error = "Wrong master password.";
    }
  }

  /** Before a sign-in is kept: an open store, the first one with its master password set. */
  async ready(): Promise<boolean> {
    const store = await credentialStore();
    if (store.state === "unlocked") return true;
    if (store.state === "locked") return await this.unlock();
    const answer = await this.#ask("create");
    if (answer === null || answer === "forgot") return false;
    await createCredentialStore(answer.password);
    return true;
  }

  /** Preferences ▸ Authentication ▸ Change Master Password: what is stored stays. */
  async change(): Promise<boolean> {
    const store = await credentialStore();
    if (store.state === "missing") return await this.ready();
    let error: string | null = null;
    for (;;) {
      const answer = await this.#ask("change", error, store.masterPassword);
      if (answer === null) return false;
      if (answer === "forgot") return await this.#forgot();
      if (await changeMasterPassword(answer.current, answer.password)) {
        this.#declined = false;
        return true;
      }
      error = "The current master password is wrong.";
    }
  }

  async #forgot(): Promise<boolean> {
    const answer = await this.#ask("forgot");
    if (answer === null || answer === "forgot") return false;
    await resetCredentialStore(answer.password);
    this.#declined = false;
    return true;
  }
}

export const masterPassword = new MasterPasswordStore();
