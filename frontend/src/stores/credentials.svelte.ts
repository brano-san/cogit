import { forgetAllCredentials, forgetCredential, listCredentials, storeCredential, type KnownCredential } from "$lib/ipc";
import { confirmation } from "$stores/confirm.svelte";
import { notices } from "$stores/notices.svelte";
import { signIn } from "$stores/sign-in.svelte";

/** Preferences ▸ Authentication: the hosts Cogit holds a sign-in for (F-082). Secrets never
    come here; the list is read again after every change, so it shows what the backend kept. */
class CredentialsStore {
  known = $state.raw<KnownCredential[]>([]);
  persistent = $state(true);

  async refresh(): Promise<void> {
    try {
      const list = await listCredentials();
      this.known = list.known;
      this.persistent = list.persistent;
    } catch (err) {
      notices.report(err, "Could not read the stored sign-ins");
    }
  }

  async add(): Promise<void> {
    const typed = await signIn.ask({ host: "", username: null, error: null }, true);
    if (typed === null) return;
    await this.#change("Could not store the sign-in", () => storeCredential(typed));
  }

  async remove(host: string): Promise<void> {
    await this.#change("Could not remove the sign-in", () => forgetCredential(host));
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
