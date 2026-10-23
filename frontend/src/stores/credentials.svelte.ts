import {
  credentialStore,
  forgetAllCredentials,
  forgetCredential,
  storeCredential,
  type KnownCredential,
  type StoreState,
} from "$lib/ipc";
import { confirmation } from "$stores/confirm.svelte";
import { masterPassword } from "$stores/master-password.svelte";
import { notices } from "$stores/notices.svelte";
import { signIn } from "$stores/sign-in.svelte";

/** Preferences ▸ Authentication: Cogit's password store and the sign-ins in it (F-082, R-891).
    Secrets never come here; the list is read again after every change, so it shows what the
    backend kept. */
class CredentialsStore {
  known = $state.raw<KnownCredential[]>([]);
  state = $state<StoreState>("missing");
  hasMasterPassword = $state(false);

  async refresh(): Promise<void> {
    try {
      const store = await credentialStore();
      this.known = store.known;
      this.state = store.state;
      this.hasMasterPassword = store.masterPassword;
    } catch (err) {
      notices.report(err, "Could not read the password store");
    }
  }

  async unlock(): Promise<void> {
    await this.#change("Could not unlock the password store", () => masterPassword.unlock());
  }

  async changeMasterPassword(): Promise<void> {
    await this.#change("Could not change the master password", () => masterPassword.change());
  }

  async add(): Promise<void> {
    await this.#change("Could not store the sign-in", async () => {
      if (!(await masterPassword.ready())) return;
      const typed = await signIn.ask({ host: "", username: null, error: null, ssh: false }, true);
      if (typed !== null) await storeCredential(typed);
    });
  }

  async remove(known: KnownCredential): Promise<void> {
    await this.#change("Could not remove the sign-in", () => forgetCredential(known.host, known.keyPath !== null));
  }

  async removeAll(): Promise<void> {
    const go = await confirmation.ask({
      title: "Remove All Sign-ins",
      message: "Remove every stored sign-in? Cogit asks again the next time a remote wants one.",
      confirm: "Remove All",
      warning: true,
      items: this.known.map((entry) => entry.host),
    });
    if (!go) return;
    await this.#change("Could not remove the sign-ins", () => forgetAllCredentials());
  }

  async #change(failure: string, change: () => Promise<unknown>): Promise<void> {
    try {
      await change();
    } catch (err) {
      notices.report(err, failure);
    }
    await this.refresh();
  }
}

export const credentials = new CredentialsStore();
